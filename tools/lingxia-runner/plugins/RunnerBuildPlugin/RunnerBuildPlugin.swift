import Foundation
import PackagePlugin

@main
struct RunnerBuildPlugin: BuildToolPlugin {
    func createBuildCommands(context: PluginContext, target: Target) throws -> [Command] {
        guard target.name == "LingXiaRunner" else {
            return []
        }

        let tool = try context.tool(named: "RunnerBuildTool")
        let outputDir = context.pluginWorkDirectoryURL.appending(path: "RunnerBuild")
        let stampPath = outputDir.appending(path: "prepared.stamp")
        let packageDir = context.package.directoryURL
        let inputFiles = collectInputFiles(packageDir: packageDir)

        return [
            .buildCommand(
                displayName: "Preparing LingXia Runner assets (Rust)",
                executable: tool.url,
                arguments: [
                    "--package-dir",
                    packageDir.path,
                    "--output-dir",
                    outputDir.path,
                ],
                environment: [:],
                inputFiles: inputFiles,
                outputFiles: [stampPath]
            )
        ]
    }

    private func collectInputFiles(packageDir: URL) -> [URL] {
        let fileManager = FileManager.default
        let projectRoot = packageDir
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .standardizedFileURL

        var inputFiles: [URL] = []
        var seen = Set<String>()

        func appendFile(_ url: URL) {
            let path = url.standardizedFileURL.path
            guard !seen.contains(path), fileManager.isReadableFile(atPath: path) else {
                return
            }

            var isDirectory: ObjCBool = false
            guard fileManager.fileExists(atPath: path, isDirectory: &isDirectory),
                  !isDirectory.boolValue
            else {
                return
            }

            seen.insert(path)
            inputFiles.append(URL(fileURLWithPath: path))
        }

        func appendFiles(in directory: URL) {
            let rootPath = directory.standardizedFileURL.path
            guard let enumerator = fileManager.enumerator(
                at: directory.standardizedFileURL,
                includingPropertiesForKeys: [.isRegularFileKey],
                options: [.skipsHiddenFiles]
            ) else {
                return
            }

            for case let fileURL as URL in enumerator {
                let path = fileURL.standardizedFileURL.path
                if path.contains("/target/") || path.contains("/.build/") {
                    continue
                }
                guard path.hasPrefix(rootPath + "/") else {
                    continue
                }
                guard let values = try? fileURL.resourceValues(forKeys: [.isRegularFileKey]),
                      values.isRegularFile == true
                else {
                    continue
                }
                appendFile(fileURL)
            }
        }

        appendFile(packageDir.appending(path: "Package.swift"))
        appendFiles(in: packageDir.appending(path: "plugins"))
        appendFiles(in: packageDir.appending(path: "runner-lib"))
        appendFile(projectRoot.appending(path: "packages/lingxia-bridge/dist/bridge-runtime.es2020.js"))
        appendFile(projectRoot.appending(path: "Cargo.toml"))
        appendFile(projectRoot.appending(path: "Cargo.lock"))
        appendFiles(in: projectRoot.appending(path: "crates"))

        return inputFiles.sorted { $0.path < $1.path }
    }
}
