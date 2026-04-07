import Foundation
import PackagePlugin

@main
struct RunnerBuildPlugin: BuildToolPlugin {
    func createBuildCommands(context: PluginContext, target: Target) throws -> [Command] {
        guard target.name == "LingXiaRunner" else {
            return []
        }

        let fileManager = FileManager.default
        let tool = try context.tool(named: "RunnerBuildTool")
        let outputDir = context.pluginWorkDirectoryURL.appending(path: "RunnerBuild")
        let stampPath = outputDir.appending(path: "prepared.stamp")
        let packageDir = context.package.directoryURL
        let runtimePackageJSON = packageDir.appending(path: "../../packages/lingxia-bridge/package.json")
        let runtimePackageLock = packageDir.appending(path: "../../packages/lingxia-bridge/package-lock.json")
        let runtimePackageSourceDir = packageDir.appending(path: "../../packages/lingxia-bridge/src")
        let runtimeRolldownConfig = packageDir.appending(path: "../../packages/lingxia-bridge/rolldown.config.js")
        let runtimeTsConfig = packageDir.appending(path: "../../packages/lingxia-bridge/tsconfig.json")
        let devtoolCargo = packageDir.appending(path: "../../crates/lingxia-devtool/Cargo.toml")
        let devtoolSourceDir = packageDir.appending(path: "../../crates/lingxia-devtool/src")

        var inputFiles = [
            packageDir.appending(path: "Package.swift"),
            packageDir.appending(path: "plugins/RunnerBuildTool/main.swift"),
            runtimePackageJSON,
            runtimePackageSourceDir,
            runtimeRolldownConfig,
            runtimeTsConfig,
            packageDir.appending(path: "../../Cargo.toml"),
            devtoolCargo,
            devtoolSourceDir,
        ]
        if fileManager.fileExists(atPath: runtimePackageLock.path) {
            inputFiles.append(runtimePackageLock)
        }

        return [
            .buildCommand(
                displayName: "Preparing LingXia Runner assets (web-runtime + Rust)",
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
}
