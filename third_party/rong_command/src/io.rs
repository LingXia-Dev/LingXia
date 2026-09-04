use crate::shell;
use rong::{
    AnyJSTypedArray, HostError, JSArrayBuffer, JSContext, JSContextService, JSFunc, JSObject,
    JSResult, JSValue,
};
use rong_stream::JSReadableStream;
use std::io::Write;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};

const STDIO_CHUNK_SIZE: usize = 8192;

#[derive(Clone)]
pub(crate) struct CapturedStdio {
    stdin: Arc<Vec<u8>>,
    stdout: Arc<Mutex<Vec<u8>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
}

impl CapturedStdio {
    #[cfg(test)]
    pub(crate) fn new(stdin: Vec<u8>) -> Self {
        Self {
            stdin: Arc::new(stdin),
            stdout: Arc::new(Mutex::new(Vec::new())),
            stderr: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn stdout_bytes(&self) -> Vec<u8> {
        self.stdout
            .lock()
            .map(|buf| buf.clone())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn stderr_bytes(&self) -> Vec<u8> {
        self.stderr
            .lock()
            .map(|buf| buf.clone())
            .unwrap_or_default()
    }
}

impl JSContextService for CapturedStdio {}

struct BufferReader {
    bytes: Arc<Vec<u8>>,
    offset: usize,
}

impl BufferReader {
    fn new(bytes: Arc<Vec<u8>>) -> Self {
        Self { bytes, offset: 0 }
    }
}

impl AsyncRead for BufferReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.offset >= self.bytes.len() {
            return Poll::Ready(Ok(()));
        }

        let remaining = &self.bytes[self.offset..];
        let count = remaining.len().min(buf.remaining());
        buf.put_slice(&remaining[..count]);
        self.offset += count;
        Poll::Ready(Ok(()))
    }
}

fn type_error(message: impl Into<String>) -> HostError {
    HostError::new(rong::error::E_TYPE, message).with_name("TypeError")
}

fn js_value_to_bytes(value: &JSValue, label: &str) -> JSResult<Vec<u8>> {
    if let Some(obj) = value.clone().into_object() {
        if let Some(typed_array) = AnyJSTypedArray::from_object(obj.clone()) {
            let bytes = typed_array
                .byte_view()
                .ok_or_else(|| type_error(format!("{label} contains an invalid TypedArray")))?;
            return Ok(bytes.to_vec());
        }
        if let Some(array_buffer) = JSArrayBuffer::from_object(obj) {
            return Ok(array_buffer.to_vec());
        }
    }

    value
        .clone()
        .to_rust::<String>()
        .map(|text| text.into_bytes())
        .map_err(|_| {
            type_error(format!(
                "{label} must be a string, ArrayBuffer, or TypedArray"
            ))
            .into()
        })
}

pub(crate) fn write_stdout_bytes_native(data: &[u8]) -> bool {
    std::io::stdout().write_all(data).is_ok()
}

pub(crate) fn write_stderr_bytes_native(data: &[u8]) -> bool {
    std::io::stderr().write_all(data).is_ok()
}

pub(crate) fn flush_stdout_native() -> bool {
    std::io::stdout().flush().is_ok()
}

pub(crate) fn flush_stderr_native() -> bool {
    std::io::stderr().flush().is_ok()
}

fn create_output_handle<FWrite, FFlush>(
    ctx: &JSContext,
    name: &'static str,
    write_bytes: FWrite,
    flush_bytes: FFlush,
) -> JSResult<JSObject>
where
    FWrite: Fn(&[u8]) -> bool + 'static,
    FFlush: Fn() -> bool + Clone + 'static,
{
    let obj = JSObject::new(ctx);
    let flush_after_write = flush_bytes.clone();

    obj.set(
        "write",
        JSFunc::new(ctx, move |value: JSValue| {
            let bytes = js_value_to_bytes(&value, name)?;
            if !write_bytes(&bytes) {
                return Err(
                    HostError::new(rong::error::E_IO, format!("Failed to write {name}")).into(),
                );
            }
            if !flush_after_write() {
                return Err(
                    HostError::new(rong::error::E_IO, format!("Failed to flush {name}")).into(),
                );
            }
            Ok(())
        })?
        .name("write")?,
    )?;

    obj.set(
        "flush",
        JSFunc::new(ctx, move || {
            if !flush_bytes() {
                return Err(
                    HostError::new(rong::error::E_IO, format!("Failed to flush {name}")).into(),
                );
            }
            Ok(())
        })?
        .name("flush")?,
    )?;

    Ok(obj)
}

fn create_default_stdin(ctx: &JSContext) -> JSResult<JSObject> {
    let stdin = JSReadableStream::from_async_reader(ctx, tokio::io::stdin(), STDIO_CHUNK_SIZE)?;
    let obj = stdin.into_object();
    shell::decorate_readable(&obj)?;
    Ok(obj)
}

fn create_captured_stdin(ctx: &JSContext, stdio: &CapturedStdio) -> JSResult<JSObject> {
    let stdin = JSReadableStream::from_async_reader(
        ctx,
        BufferReader::new(stdio.stdin.clone()),
        STDIO_CHUNK_SIZE,
    )?;
    let obj = stdin.into_object();
    shell::decorate_readable(&obj)?;
    Ok(obj)
}

fn create_default_stdout(ctx: &JSContext) -> JSResult<JSObject> {
    create_output_handle(
        ctx,
        "stdout",
        write_stdout_bytes_native,
        flush_stdout_native,
    )
}

fn create_default_stderr(ctx: &JSContext) -> JSResult<JSObject> {
    create_output_handle(
        ctx,
        "stderr",
        write_stderr_bytes_native,
        flush_stderr_native,
    )
}

fn create_captured_stdout(ctx: &JSContext, stdio: &CapturedStdio) -> JSResult<JSObject> {
    let bytes = stdio.stdout.clone();
    create_output_handle(
        ctx,
        "stdout",
        move |chunk| {
            bytes
                .lock()
                .map(|mut buf| {
                    buf.extend_from_slice(chunk);
                    true
                })
                .unwrap_or(false)
        },
        || true,
    )
}

fn create_captured_stderr(ctx: &JSContext, stdio: &CapturedStdio) -> JSResult<JSObject> {
    let bytes = stdio.stderr.clone();
    create_output_handle(
        ctx,
        "stderr",
        move |chunk| {
            bytes
                .lock()
                .map(|mut buf| {
                    buf.extend_from_slice(chunk);
                    true
                })
                .unwrap_or(false)
        },
        || true,
    )
}

#[cfg(test)]
pub(crate) fn install_captured_stdio(ctx: &JSContext, stdin: Vec<u8>) -> CapturedStdio {
    let stdio = CapturedStdio::new(stdin);
    ctx.set_service(stdio.clone());
    stdio
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let rong = ctx.host_namespace();

    if let Some(stdio) = ctx.get_service::<CapturedStdio>() {
        rong.set("stdin", create_captured_stdin(ctx, stdio)?)?;
        rong.set("stdout", create_captured_stdout(ctx, stdio)?)?;
        rong.set("stderr", create_captured_stderr(ctx, stdio)?)?;
        return Ok(());
    }

    rong.set("stdin", create_default_stdin(ctx)?)?;
    rong.set("stdout", create_default_stdout(ctx)?)?;
    rong.set("stderr", create_default_stderr(ctx)?)?;
    Ok(())
}
