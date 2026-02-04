//! SRP (Secure Remote Password) client implementation for Apple authentication.
//!
//! Implements SRP-6a with 2048-bit parameters as used by Apple's GrandSlam.

use aes::cipher::{BlockDecryptMut, KeyIvInit};
use anyhow::{Result, anyhow};
use hmac::{Hmac, Mac};
use num_bigint::BigUint;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// SRP 2048-bit prime (N) used by Apple
const N_HEX: &str = "AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";

/// SRP generator (g)
const G: u32 = 2;

/// SRP client for Apple authentication
pub struct SrpClient {
    /// Client private key (a)
    private_key: BigUint,
    /// Client public key (A = g^a mod N)
    public_key: BigUint,
    /// Prime modulus
    n: BigUint,
    /// Generator
    g: BigUint,
    /// Session key (set after process_challenge)
    session_key: Option<Vec<u8>>,
    /// Expected HAMK (set after process_challenge)
    expected_hamk: Option<Vec<u8>>,
}

impl SrpClient {
    /// Create a new SRP client with random private key
    pub fn new() -> Self {
        let n = BigUint::parse_bytes(N_HEX.as_bytes(), 16).expect("Invalid N constant");
        let g = BigUint::from(G);

        // Generate random 256-bit private key
        let mut private_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut private_bytes);
        let private_key = BigUint::from_bytes_be(&private_bytes);

        // Compute public key: A = g^a mod N
        let public_key = g.modpow(&private_key, &n);

        Self {
            private_key,
            public_key,
            n,
            g,
            session_key: None,
            expected_hamk: None,
        }
    }

    /// Get the client public key (A) as bytes
    pub fn public_key_bytes(&self) -> Vec<u8> {
        pad_to_n_size(&self.public_key.to_bytes_be(), &self.n)
    }

    /// Process the server challenge and compute session key and proof
    ///
    /// Returns M1 (client proof)
    pub fn process_challenge(
        &mut self,
        username: &str,
        password: &str,
        salt: &[u8],
        iterations: u32,
        is_legacy_protocol: bool,
        server_public_key: &[u8],
    ) -> Result<Vec<u8>> {
        let b = BigUint::from_bytes_be(server_public_key);

        // Verify B != 0 mod N
        if &b % &self.n == BigUint::from(0u32) {
            return Err(anyhow!("Invalid server public key (B = 0 mod N)"));
        }

        // Compute u = SHA256(pad(A) || pad(B))
        let a_padded = pad_to_n_size(&self.public_key.to_bytes_be(), &self.n);
        let b_padded = pad_to_n_size(&b.to_bytes_be(), &self.n);
        let u = BigUint::from_bytes_be(&sha256_concat(&[&a_padded, &b_padded]));

        if u == BigUint::from(0u32) {
            return Err(anyhow!("Invalid u parameter (u = 0)"));
        }

        // Compute k = SHA256(pad(N) || pad(g))
        let n_padded = pad_to_n_size(&self.n.to_bytes_be(), &self.n);
        let g_padded = pad_to_n_size(&self.g.to_bytes_be(), &self.n);
        let k = BigUint::from_bytes_be(&sha256_concat(&[&n_padded, &g_padded]));

        // Derive password key using PBKDF2
        let password_hash = sha256(password.as_bytes());
        let pbkdf_input = if is_legacy_protocol {
            // Legacy protocol: use hex string of hash
            password_hash
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
                .into_bytes()
        } else {
            // Modern protocol: use raw hash
            password_hash.to_vec()
        };

        let mut derived = [0u8; 32];
        pbkdf2_hmac::<Sha256>(&pbkdf_input, salt, iterations, &mut derived);

        // Compute x = SHA256(salt || SHA256(":" || derived))
        let inner_hash = sha256_concat(&[b":", &derived]);
        let x = BigUint::from_bytes_be(&sha256_concat(&[salt, &inner_hash]));

        // Compute S = (B - k * g^x)^(a + u*x) mod N
        let g_x = self.g.modpow(&x, &self.n);
        let k_g_x = (&k * &g_x) % &self.n;

        // Handle potential underflow: if B < k*g^x, add N
        let base = if b >= k_g_x {
            &b - &k_g_x
        } else {
            &b + &self.n - &k_g_x
        };

        let exp = &self.private_key + &u * &x;
        let s = base.modpow(&exp, &self.n);

        // Session key K = SHA256(S)
        let session_key = sha256(&pad_to_n_size(&s.to_bytes_be(), &self.n)).to_vec();

        // Compute M1 = SHA256(xor(H(N), H(g)) || H(username) || salt || A || B || K)
        let n_hash = sha256(&pad_to_n_size(&self.n.to_bytes_be(), &self.n));
        let g_hash = sha256(&pad_to_n_size(&self.g.to_bytes_be(), &self.n));
        let ng_xor: Vec<u8> = n_hash
            .iter()
            .zip(g_hash.iter())
            .map(|(a, b)| a ^ b)
            .collect();
        let username_hash = sha256(username.as_bytes());

        let m1 = sha256_concat(&[
            &ng_xor,
            &username_hash,
            salt,
            &a_padded,
            &b_padded,
            &session_key,
        ]);

        // Compute expected HAMK = SHA256(A || M1 || K)
        let hamk = sha256_concat(&[&a_padded, &m1, &session_key]);

        self.session_key = Some(session_key);
        self.expected_hamk = Some(hamk.to_vec());

        Ok(m1.to_vec())
    }

    /// Verify server's response (M2/HAMK)
    pub fn verify_server_proof(&self, m2: &[u8]) -> bool {
        self.expected_hamk.as_ref().map_or(false, |h| h == m2)
    }

    /// Get session key
    pub fn session_key(&self) -> &[u8] {
        self.session_key.as_ref().map_or(&[], |k| k.as_slice())
    }

    /// Decrypt the encrypted response from auth complete
    pub fn decrypt_response(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        let session_key = self
            .session_key
            .as_ref()
            .ok_or_else(|| anyhow!("Session key not available"))?;

        // Derive encryption key and IV
        let key = hmac_sha256(session_key, b"extra data key:");
        let iv_full = hmac_sha256(session_key, b"extra data iv:");
        let iv = &iv_full[..16];

        // Decrypt using AES-256-CBC
        let cipher = Aes256CbcDec::new_from_slices(&key, iv)
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        let mut buffer = encrypted.to_vec();
        let decrypted = cipher
            .decrypt_padded_mut::<aes::cipher::block_padding::Pkcs7>(&mut buffer)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        Ok(decrypted.to_vec())
    }
}

/// Pad bytes to N size (256 bytes for 2048-bit)
fn pad_to_n_size(bytes: &[u8], n: &BigUint) -> Vec<u8> {
    let n_size = (n.bits() as usize + 7) / 8;
    if bytes.len() >= n_size {
        bytes.to_vec()
    } else {
        let mut padded = vec![0u8; n_size - bytes.len()];
        padded.extend_from_slice(bytes);
        padded
    }
}

/// SHA256 hash
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// SHA256 hash of concatenated data
fn sha256_concat(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// HMAC-SHA256
fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srp_client_creation() {
        let client = SrpClient::new();
        let pk = client.public_key_bytes();
        assert_eq!(pk.len(), 256); // 2048 bits = 256 bytes
    }
}
