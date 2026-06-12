import Foundation

enum RunnerBuildToolError: Error, CustomStringConvertible {
    case invalidArgs(String)
    case missingFile(String)
    case missingCommand(String)
    case unsupportedValue(String)
    case commandFailed(command: String, code: Int32, output: String)

    var description: String {
        switch self {
        case .invalidArgs(let message):
            return "Invalid arguments: \(message)"
        case .missingFile(let path):
            return "Required file not found: \(path)"
        case .missingCommand(let message):
            return "Required command not found: \(message)"
        case .unsupportedValue(let message):
            return "Unsupported value: \(message)"
        case .commandFailed(let command, let code, let output):
            if output.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                return "Command failed (\(code)): \(command)"
            }
            return "Command failed (\(code)): \(command)\n\(output)"
        }
    }
}

struct Options {
    let packageDir: String
    let outputDir: String

    static func parse(_ args: [String]) throws -> Options {
        var packageDir: String?
        var outputDir: String?
        var index = 0

        while index < args.count {
            let arg = args[index]
            switch arg {
            case "--package-dir":
                guard index + 1 < args.count else {
                    throw RunnerBuildToolError.invalidArgs("--package-dir requires a value")
                }
                packageDir = args[index + 1]
                index += 2
            case "--output-dir":
                guard index + 1 < args.count else {
                    throw RunnerBuildToolError.invalidArgs("--output-dir requires a value")
                }
                outputDir = args[index + 1]
                index += 2
            default:
                throw RunnerBuildToolError.invalidArgs("Unknown argument: \(arg)")
            }
        }

        guard let packageDir, let outputDir else {
            throw RunnerBuildToolError.invalidArgs("Expected --package-dir and --output-dir")
        }

        return Options(packageDir: packageDir, outputDir: outputDir)
    }
}

struct ToolPaths {
    let cargo: String
}

@main
struct RunnerBuildTool {
    static func main() {
        do {
            try run()
        } catch {
            let message = String(describing: error)
            fputs("error: \(message)\n", stderr)
            if shouldSuggestDisableSandbox(error: error) {
                fputs(
                    "hint: This plugin builds artifacts outside the package directory.\n" +
                    "      Run with sandbox disabled:\n" +
                    "      swift build --disable-sandbox\n",
                    stderr
                )
            }
            exit(1)
        }
    }

    private static func run() throws {
        let args = Array(CommandLine.arguments.dropFirst())
        let options = try Options.parse(args)
        let fileManager = FileManager.default

        try fileManager.createDirectory(
            atPath: options.outputDir,
            withIntermediateDirectories: true,
            attributes: nil
        )

        let projectRoot = resolveProjectRoot(packageDir: options.packageDir)
        let buildConfig = resolveBuildConfig()
        let tools = try resolveToolPaths()
        let baseEnvironment = buildBaseEnvironment(tools: tools)

        print("[runner-plugin] projectRoot=\(projectRoot)")
        print("[runner-plugin] buildConfig=\(buildConfig)")
        print("[runner-plugin] cargo=\(tools.cargo)")

        try buildRustLibrary(
            projectRoot: projectRoot,
            buildConfig: buildConfig,
            cargoPath: tools.cargo,
            baseEnvironment: baseEnvironment
        )
        try syncBridgeRuntimeAsset(projectRoot: projectRoot, packageDir: options.packageDir)

        let stampPath = (options.outputDir as NSString).appendingPathComponent("prepared.stamp")
        let formatter = ISO8601DateFormatter()
        let stamp = "prepared at \(formatter.string(from: Date()))\n"
        try stamp.write(toFile: stampPath, atomically: true, encoding: .utf8)
    }

    private static func resolveProjectRoot(packageDir: String) -> String {
        if let explicit = ProcessInfo.processInfo.environment["LINGXIA_PROJECT_ROOT"],
           !explicit.isEmpty,
           let resolved = findMonorepoRoot(startPath: explicit)
        {
            return resolved
        }

        if let resolved = findMonorepoRoot(startPath: packageDir) {
            return resolved
        }

        if let explicit = ProcessInfo.processInfo.environment["LINGXIA_PROJECT_ROOT"],
           !explicit.isEmpty
        {
            return normalizePath(explicit)
        }
        let packageURL = URL(fileURLWithPath: packageDir)
        return packageURL
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .path
    }

    private static func findMonorepoRoot(startPath: String) -> String? {
        let fileManager = FileManager.default
        var currentURL = URL(fileURLWithPath: normalizePath(startPath), isDirectory: true)

        while true {
            let candidate = currentURL.path
            let runnerLibCargo = pathJoin(candidate, "tools/lingxia-runner/macos/runner-lib/Cargo.toml")
            let sdkPackage = pathJoin(candidate, "lingxia-sdk/apple/Package.swift")

            if fileManager.fileExists(atPath: runnerLibCargo)
                && fileManager.fileExists(atPath: sdkPackage)
            {
                return candidate
            }

            let parent = currentURL.deletingLastPathComponent()
            if parent.path == currentURL.path {
                return nil
            }
            currentURL = parent
        }
    }

    private static func resolveBuildConfig() -> String {
        let value = (ProcessInfo.processInfo.environment["LINGXIA_BUILD_CONFIG"] ?? "release")
            .lowercased()
        if value == "debug" || value == "release" {
            return value
        }
        return "release"
    }

    private static func resolveToolPaths() throws -> ToolPaths {
        let cargo = try resolveCommand(
            name: "cargo",
            envOverrideKey: "RUNNER_CARGO_BIN",
            fallbackPaths: [
                "\(homeDirectory())/.cargo/bin/cargo",
                "/opt/homebrew/bin/cargo",
                "/usr/local/bin/cargo",
                "/usr/bin/cargo",
            ]
        )
        return ToolPaths(cargo: cargo)
    }

    private static func resolveCommand(
        name: String,
        envOverrideKey: String,
        fallbackPaths: [String]
    ) throws -> String {
        let fm = FileManager.default

        if let explicit = ProcessInfo.processInfo.environment[envOverrideKey],
           !explicit.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        {
            let normalized = normalizePath(explicit)
            if fm.isExecutableFile(atPath: normalized) {
                return normalized
            }
            throw RunnerBuildToolError.missingCommand("\(envOverrideKey)=\(normalized)")
        }

        var candidates: [String] = []
        if let path = ProcessInfo.processInfo.environment["PATH"], !path.isEmpty {
            for entry in path.split(separator: ":") {
                candidates.append((String(entry) as NSString).appendingPathComponent(name))
            }
        }
        candidates.append(contentsOf: fallbackPaths)

        var seen = Set<String>()
        for candidate in candidates {
            let normalized = normalizePath(candidate)
            if seen.contains(normalized) {
                continue
            }
            seen.insert(normalized)
            if fm.isExecutableFile(atPath: normalized) {
                return normalized
            }
        }

        throw RunnerBuildToolError.missingCommand(name)
    }

    private static func buildBaseEnvironment(tools: ToolPaths) -> [String: String] {
        var environment = ProcessInfo.processInfo.environment

        let extraPathSegments = [
            (tools.cargo as NSString).deletingLastPathComponent,
            "\(homeDirectory())/.cargo/bin",
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
        ]

        var segments: [String] = []
        var seen = Set<String>()

        func appendSegment(_ value: String) {
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty || seen.contains(trimmed) {
                return
            }
            seen.insert(trimmed)
            segments.append(trimmed)
        }

        if let currentPath = environment["PATH"] {
            for segment in currentPath.split(separator: ":") {
                appendSegment(String(segment))
            }
        }
        for segment in extraPathSegments {
            appendSegment(segment)
        }

        environment["PATH"] = segments.joined(separator: ":")
        return environment
    }

    private static func buildRustLibrary(
        projectRoot: String,
        buildConfig: String,
        cargoPath: String,
        baseEnvironment: [String: String]
    ) throws {
        let targetTriple = try resolveRustTargetTriple(baseEnvironment: baseEnvironment)
        let macosDeploymentTarget = "12.0"

        var args = [
            "rustc",
            "--lib",
            "--crate-type=staticlib",
            "--target",
            targetTriple,
            "-p",
            "lingxia-runner-lib",
        ]
        if buildConfig == "release" {
            args.insert("--release", at: 5)
        }

        let libDir = pathJoin(projectRoot, "target/\(targetTriple)/\(buildConfig)")
        let prebuiltLib = try? resolveBuiltStaticLibrary(libDir: libDir)

        print("[runner-plugin] building Rust staticlib (\(targetTriple), \(buildConfig))")
        do {
            _ = try runCommand(
                executable: cargoPath,
                args: args,
                currentDir: projectRoot,
                baseEnvironment: baseEnvironment,
                envOverrides: ["MACOSX_DEPLOYMENT_TARGET": macosDeploymentTarget]
            )
        } catch {
            guard let prebuiltLib else {
                throw error
            }
            print("[runner-plugin] warning: cargo build failed, reusing existing staticlib: \(prebuiltLib)")
        }

        let src = try resolveBuiltStaticLibrary(libDir: libDir)
        let dst = pathJoin(libDir, "liblingxia.a")
        try copyIfChanged(from: src, to: dst)
        print("[runner-plugin] liblingxia.a updated: \(dst)")
    }

    private static func syncBridgeRuntimeAsset(projectRoot: String, packageDir: String) throws {
        let source = pathJoin(
            projectRoot,
            "packages/lingxia-bridge/dist/bridge-runtime.es2020.js"
        )
        guard FileManager.default.fileExists(atPath: source) else {
            throw RunnerBuildToolError.missingFile(source)
        }

        let destination = pathJoin(
            packageDir,
            "Sources/Resources/bridge-runtime.js"
        )
        try copyIfChanged(from: source, to: destination)
        print("[runner-plugin] bridge-runtime.js updated: \(destination)")
    }

    private static func resolveRustTargetTriple(baseEnvironment: [String: String]) throws -> String {
        let env = ProcessInfo.processInfo.environment
        if let configured = env["RUNNER_TARGET_TRIPLE"]?.trimmingCharacters(in: .whitespacesAndNewlines),
           !configured.isEmpty
        {
            return try normalizeRustTargetTriple(configured)
        }

        let hostArch = try runCommand(
            executable: "/usr/bin/uname",
            args: ["-m"],
            baseEnvironment: baseEnvironment
        ).trimmingCharacters(in: .whitespacesAndNewlines)
        return hostArch == "arm64" ? "aarch64-apple-darwin" : "x86_64-apple-darwin"
    }

    private static func normalizeRustTargetTriple(_ rawValue: String) throws -> String {
        let value = rawValue.lowercased()
        switch value {
        case "aarch64-apple-darwin", "arm64-apple-darwin":
            return "aarch64-apple-darwin"
        case "x86_64-apple-darwin":
            return "x86_64-apple-darwin"
        default:
            break
        }

        if value.hasPrefix("arm64-apple-macosx") || value.hasPrefix("aarch64-apple-macosx") {
            return "aarch64-apple-darwin"
        }
        if value.hasPrefix("x86_64-apple-macosx") {
            return "x86_64-apple-darwin"
        }

        throw RunnerBuildToolError.unsupportedValue(
            "RUNNER_TARGET_TRIPLE=\(rawValue) (expected macOS arm64/x86_64 target triple)"
        )
    }

    private static func runCommand(
        executable: String,
        args: [String],
        currentDir: String? = nil,
        baseEnvironment: [String: String],
        envOverrides: [String: String] = [:]
    ) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = args
        if let currentDir {
            process.currentDirectoryURL = URL(fileURLWithPath: currentDir)
        }

        var environment = baseEnvironment
        for (key, value) in envOverrides {
            environment[key] = value
        }
        process.environment = environment

        let fileManager = FileManager.default
        let tempRoot = fileManager.temporaryDirectory
        let stdoutURL = tempRoot.appendingPathComponent(UUID().uuidString).appendingPathExtension("out")
        let stderrURL = tempRoot.appendingPathComponent(UUID().uuidString).appendingPathExtension("err")
        fileManager.createFile(atPath: stdoutURL.path, contents: nil)
        fileManager.createFile(atPath: stderrURL.path, contents: nil)
        let stdoutHandle = try FileHandle(forWritingTo: stdoutURL)
        let stderrHandle = try FileHandle(forWritingTo: stderrURL)
        process.standardOutput = stdoutHandle
        process.standardError = stderrHandle

        try process.run()
        process.waitUntilExit()
        try stdoutHandle.close()
        try stderrHandle.close()

        let stdoutData = (try? Data(contentsOf: stdoutURL)) ?? Data()
        let stderrData = (try? Data(contentsOf: stderrURL)) ?? Data()
        let combined = stdoutData + stderrData
        let output = String(decoding: combined, as: UTF8.self)
        if !output.isEmpty {
            print(output, terminator: "")
        }

        try? fileManager.removeItem(at: stdoutURL)
        try? fileManager.removeItem(at: stderrURL)

        if process.terminationStatus != 0 {
            let printable = ([executable] + args).joined(separator: " ")
            throw RunnerBuildToolError.commandFailed(
                command: printable,
                code: process.terminationStatus,
                output: output
            )
        }
        return output
    }

    private static func copyIfChanged(from source: String, to destination: String) throws {
        let fm = FileManager.default
        let sourceURL = URL(fileURLWithPath: source)
        let destinationURL = URL(fileURLWithPath: destination)
        let sourceData = try Data(contentsOf: sourceURL)

        if let existingData = try? Data(contentsOf: destinationURL), existingData == sourceData {
            return
        }

        let destinationDir = (destination as NSString).deletingLastPathComponent
        try fm.createDirectory(
            atPath: destinationDir,
            withIntermediateDirectories: true,
            attributes: nil
        )
        try sourceData.write(to: destinationURL, options: .atomic)
    }

    private static func resolveBuiltStaticLibrary(libDir: String) throws -> String {
        let fm = FileManager.default
        let directCandidates = [
            pathJoin(libDir, "liblingxia.a"),
            pathJoin(libDir, "liblingxia_lib.a"),
        ]
        for candidate in directCandidates where fm.fileExists(atPath: candidate) {
            return candidate
        }

        let depsDir = pathJoin(libDir, "deps")
        guard let entries = try? fm.contentsOfDirectory(atPath: depsDir) else {
            throw RunnerBuildToolError.missingFile(directCandidates[0])
        }

        let candidates = entries
            .filter { $0.hasPrefix("liblingxia-") && $0.hasSuffix(".a") }
            .map { pathJoin(depsDir, $0) }

        guard let latest = candidates.max(by: { modificationDate(atPath: $0) < modificationDate(atPath: $1) }) else {
            throw RunnerBuildToolError.missingFile(pathJoin(depsDir, "liblingxia-*.a"))
        }
        return latest
    }

    private static func modificationDate(atPath path: String) -> Date {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: path),
              let modified = attributes[.modificationDate] as? Date
        else {
            return .distantPast
        }
        return modified
    }

    private static func newestModificationDate(atPath path: String) -> Date {
        let fileManager = FileManager.default
        guard fileManager.fileExists(atPath: path) else {
            return .distantPast
        }

        var isDirectory: ObjCBool = false
        fileManager.fileExists(atPath: path, isDirectory: &isDirectory)
        if !isDirectory.boolValue {
            return modificationDate(atPath: path)
        }

        let enumerator = fileManager.enumerator(atPath: path)
        var newest = modificationDate(atPath: path)
        while let next = enumerator?.nextObject() as? String {
            let fullPath = pathJoin(path, next)
            let modified = modificationDate(atPath: fullPath)
            if modified > newest {
                newest = modified
            }
        }
        return newest
    }

    private static func shouldSuggestDisableSandbox(error: Error) -> Bool {
        let text = String(describing: error).lowercased()
        return text.contains("operation not permitted")
            || text.contains("sandbox")
            || text.contains("permission denied")
    }

    private static func pathJoin(_ left: String, _ right: String) -> String {
        (left as NSString).appendingPathComponent(right)
    }

    private static func normalizePath(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
    }

    private static func homeDirectory() -> String {
        NSHomeDirectory()
    }
}
