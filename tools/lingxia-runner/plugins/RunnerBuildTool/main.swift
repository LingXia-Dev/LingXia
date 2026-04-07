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
    let npm: String
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
        let runtimeTarget = ProcessInfo.processInfo.environment["RUNNER_RUNTIME_TARGET"] ?? "es2020"
        let runtimePlatform = ProcessInfo.processInfo.environment["RUNNER_RUNTIME_PLATFORM"] ?? "desktop"
        let tools = try resolveToolPaths()
        let baseEnvironment = buildBaseEnvironment(tools: tools)

        print("[runner-plugin] projectRoot=\(projectRoot)")
        print("[runner-plugin] buildConfig=\(buildConfig)")
        print("[runner-plugin] runtime=\(runtimeTarget) platform=\(runtimePlatform)")
        print("[runner-plugin] npm=\(tools.npm)")
        print("[runner-plugin] cargo=\(tools.cargo)")

        try buildWebRuntime(
            projectRoot: projectRoot,
            packageDir: options.packageDir,
            outputDir: options.outputDir,
            runtimeTarget: runtimeTarget,
            runtimePlatform: runtimePlatform,
            npmPath: tools.npm,
            baseEnvironment: baseEnvironment
        )
        try buildRustLibrary(
            projectRoot: projectRoot,
            buildConfig: buildConfig,
            cargoPath: tools.cargo,
            baseEnvironment: baseEnvironment
        )

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
        return packageURL.deletingLastPathComponent().deletingLastPathComponent().path
    }

    private static func findMonorepoRoot(startPath: String) -> String? {
        let fileManager = FileManager.default
        var currentURL = URL(fileURLWithPath: normalizePath(startPath), isDirectory: true)

        while true {
            let candidate = currentURL.path
            let bridgePackage = pathJoin(candidate, "packages/lingxia-bridge/package.json")
            let devtoolCargo = pathJoin(candidate, "crates/lingxia-devtool/Cargo.toml")
            let sdkPackage = pathJoin(candidate, "lingxia-sdk/apple/Package.swift")

            if fileManager.fileExists(atPath: bridgePackage)
                && fileManager.fileExists(atPath: devtoolCargo)
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
        let npm = try resolveCommand(
            name: "npm",
            envOverrideKey: "RUNNER_NPM_BIN",
            fallbackPaths: ["/opt/homebrew/bin/npm", "/usr/local/bin/npm", "/usr/bin/npm"]
        )
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
        return ToolPaths(npm: npm, cargo: cargo)
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
            (tools.npm as NSString).deletingLastPathComponent,
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

    private static func buildWebRuntime(
        projectRoot: String,
        packageDir: String,
        outputDir: String,
        runtimeTarget: String,
        runtimePlatform: String,
        npmPath: String,
        baseEnvironment: [String: String]
    ) throws {
        let runtimeDir = pathJoin(projectRoot, "packages/lingxia-bridge")
        let packageJSON = pathJoin(runtimeDir, "package.json")
        guard FileManager.default.fileExists(atPath: packageJSON) else {
            throw RunnerBuildToolError.missingFile(packageJSON)
        }

        try ensureRuntimeDependencies(
            runtimeDir: runtimeDir,
            npmPath: npmPath,
            baseEnvironment: baseEnvironment
        )

        let script: String
        let distRuntime: String
        let runtimeOutputDir = pathJoin(outputDir, "runtime-dist")
        let prebuiltRuntime: String
        switch runtimeTarget {
        case "es2020":
            script = "build:es2020"
            distRuntime = pathJoin(runtimeOutputDir, "runtime.es2020.js")
            prebuiltRuntime = pathJoin(runtimeDir, "dist/runtime.es2020.js")
        case "es5":
            script = "build:es5"
            distRuntime = pathJoin(runtimeOutputDir, "runtime.es5.js")
            prebuiltRuntime = pathJoin(runtimeDir, "dist/runtime.es5.js")
        default:
            throw RunnerBuildToolError.unsupportedValue(
                "RUNNER_RUNTIME_TARGET=\(runtimeTarget) (expected es2020 or es5)"
            )
        }

        switch runtimePlatform {
        case "all", "desktop", "mobile":
            break
        default:
            throw RunnerBuildToolError.unsupportedValue(
                "RUNNER_RUNTIME_PLATFORM=\(runtimePlatform) (expected all, desktop, or mobile)"
            )
        }

        var runtimeEnv: [String: String] = [:]
        if runtimePlatform != "all" {
            runtimeEnv["LX_RUNTIME_PLATFORM"] = runtimePlatform
        }
        runtimeEnv["RUNTIME_OUTPUT_DIR"] = runtimeOutputDir

        try FileManager.default.createDirectory(
            atPath: runtimeOutputDir,
            withIntermediateDirectories: true,
            attributes: nil
        )

        if shouldReusePrebuiltRuntime(prebuiltRuntime, runtimeDir: runtimeDir) {
            try copyIfChanged(from: prebuiltRuntime, to: distRuntime)
            print("[runner-plugin] reusing prebuilt web-runtime: \(prebuiltRuntime)")
        } else {
            print("[runner-plugin] building web-runtime (\(script))")
            _ = try runCommand(
                executable: npmPath,
                args: ["run", script],
                currentDir: runtimeDir,
                baseEnvironment: baseEnvironment,
                envOverrides: runtimeEnv
            )
        }

        guard FileManager.default.fileExists(atPath: distRuntime) else {
            throw RunnerBuildToolError.missingFile(distRuntime)
        }

        let sdkRuntimePath = pathJoin(projectRoot, "lingxia-sdk/apple/Sources/Resources/runtime.js")
        let runnerRuntimePath = pathJoin(packageDir, "Sources/Resources/runtime.js")
        try copyIfChanged(from: distRuntime, to: sdkRuntimePath)
        try copyIfChanged(from: distRuntime, to: runnerRuntimePath)
        print("[runner-plugin] runtime.js updated")
    }

    private static func shouldReusePrebuiltRuntime(_ runtimePath: String, runtimeDir: String) -> Bool {
        let fileManager = FileManager.default
        guard fileManager.fileExists(atPath: runtimePath) else {
            return false
        }

        let prebuiltModified = modificationDate(atPath: runtimePath)
        let inputs = [
            pathJoin(runtimeDir, "package.json"),
            pathJoin(runtimeDir, "rolldown.config.js"),
            pathJoin(runtimeDir, "tsconfig.json"),
            pathJoin(runtimeDir, "src"),
        ]

        for input in inputs {
            if newestModificationDate(atPath: input) > prebuiltModified {
                return false
            }
        }

        return true
    }

    private static func ensureRuntimeDependencies(
        runtimeDir: String,
        npmPath: String,
        baseEnvironment: [String: String]
    ) throws {
        let packageJSON = pathJoin(runtimeDir, "package.json")
        let packageLock = pathJoin(runtimeDir, "package-lock.json")
        let nodeModules = pathJoin(runtimeDir, "node_modules")

        let nodeModulesExists = FileManager.default.fileExists(atPath: nodeModules)
        let nodeModulesModified = modificationDate(atPath: nodeModules)
        let packageModified = modificationDate(atPath: packageJSON)
        let lockModified = modificationDate(atPath: packageLock)

        let shouldInstall = !nodeModulesExists
            || packageModified > nodeModulesModified
            || lockModified > nodeModulesModified
        guard shouldInstall else {
            return
        }

        let installArgs = FileManager.default.fileExists(atPath: packageLock)
            ? ["ci"]
            : ["install"]
        print("[runner-plugin] installing web-runtime dependencies (\(installArgs.joined(separator: " ")))")
        _ = try runCommand(
            executable: npmPath,
            args: installArgs,
            currentDir: runtimeDir,
            baseEnvironment: baseEnvironment
        )
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
            "--crate-type=staticlib",
            "--target",
            targetTriple,
            "-p",
            "lingxia-devtool",
        ]
        if buildConfig == "release" {
            args.insert("--release", at: 4)
        }

        if let features = ProcessInfo.processInfo.environment["LXAPP_FEATURES"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
           !features.isEmpty
        {
            args.append(contentsOf: ["--features", features])
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
