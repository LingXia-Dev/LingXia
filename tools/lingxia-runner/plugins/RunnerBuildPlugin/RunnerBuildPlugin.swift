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
        let devtoolCargo = packageDir.appending(path: "../../crates/lingxia-devtool/Cargo.toml")
        let devtoolSourceDir = packageDir.appending(path: "../../crates/lingxia-devtool/src")

        let inputFiles = [
            packageDir.appending(path: "Package.swift"),
            packageDir.appending(path: "plugins/RunnerBuildTool/main.swift"),
            packageDir.appending(path: "../../Cargo.toml"),
            devtoolCargo,
            devtoolSourceDir,
        ]

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
}
