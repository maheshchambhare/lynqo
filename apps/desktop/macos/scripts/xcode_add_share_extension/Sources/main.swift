import Foundation
import XcodeProj
import PathKit

func main() {
    let args = CommandLine.arguments
    guard args.count >= 2 else {
        print("Usage: swift run xcode_add_share_extension <Path/To/Project.xcodeproj>")
        exit(1)
    }

    let projectPath = Path(args[1])
    print("Opening Xcode Project: \(projectPath)")

    do {
        let xcodeproj = try XcodeProj(path: projectPath)
        let pbxproj = xcodeproj.pbxproj

        guard let rootObject = try pbxproj.rootProject() else {
            print("Error: Could not find root project")
            exit(1)
        }

        // Check if ShareExtension already exists
        if pbxproj.nativeTargets.contains(where: { $0.name == "ShareExtension" }) {
            print("ShareExtension target already exists in the project.")
            exit(0)
        }

        guard let runnerTarget = pbxproj.nativeTargets.first(where: { $0.name == "Runner" }) else {
            print("Error: Could not find target 'Runner'")
            exit(1)
        }

        print("Adding ShareExtension target...")

        // 1. Create a PBXGroup for ShareExtension files
        let mainGroup = rootObject.mainGroup
        let extGroup = try mainGroup?.addGroup(named: "ShareExtension").last

        // 2. Add File References to Group
        let extPath = projectPath.parent() + "ShareExtension"
        let swiftFileRef = try extGroup?.addFile(at: extPath + "ShareViewController.swift", sourceRoot: projectPath.parent())
        let plistFileRef = try extGroup?.addFile(at: extPath + "Info.plist", sourceRoot: projectPath.parent())
        let entFileRef = try extGroup?.addFile(at: extPath + "ShareExtension.entitlements", sourceRoot: projectPath.parent())

        // 3. Create Product File Reference (ShareExtension.appex)
        guard let productsGroup = mainGroup?.children.first(where: { $0.name == "Products" || $0.path == "Products" }) as? PBXGroup else {
            print("Error: Could not find 'Products' group")
            exit(1)
        }

        let appexRef = PBXFileReference(
            sourceTree: .buildProductsDir,
            name: "ShareExtension.appex",
            explicitFileType: "wrapper.app-extension",
            path: "ShareExtension.appex"
        )
        pbxproj.add(object: appexRef)
        productsGroup.children.append(appexRef)

        // 4. Create Build Configurations for the target matching Runner's configuration names
        let runnerConfigs = runnerTarget.buildConfigurationList?.buildConfigurations ?? []
        var configs: [XCBuildConfiguration] = []
        for runnerConfig in runnerConfigs {
            let config = XCBuildConfiguration(
                name: runnerConfig.name,
                buildSettings: [
                    "PRODUCT_BUNDLE_IDENTIFIER": "com.lynqo.lynqo.ShareExtension",
                    "INFOPLIST_FILE": "ShareExtension/Info.plist",
                    "CODE_SIGN_ENTITLEMENTS": "ShareExtension/ShareExtension.entitlements",
                    "CODE_SIGN_STYLE": "Automatic",
                    "MACOSX_DEPLOYMENT_TARGET": "10.15",
                    "LD_RUNPATH_SEARCH_PATHS": "$(inherited) @executable_path/../Frameworks @executable_path/../Frameworks",
                    "SKIP_INSTALL": "YES",
                    "PRODUCT_NAME": "ShareExtension",
                    "GENERATE_INFOPLIST_FILE": "NO",
                    "SWIFT_VERSION": "5.0",
                ]
            )
            pbxproj.add(object: config)
            configs.append(config)
        }

        let configList = XCConfigurationList(buildConfigurations: configs, defaultConfigurationName: "Release")
        pbxproj.add(object: configList)

        // 5. Create PBXNativeTarget
        let extTarget = PBXNativeTarget(
            name: "ShareExtension",
            buildConfigurationList: configList,
            buildPhases: [],
            buildRules: [],
            dependencies: [],
            productName: "ShareExtension",
            productType: .appExtension
        )
        extTarget.product = appexRef
        pbxproj.add(object: extTarget)
        rootObject.targets.append(extTarget)

        // 6. Add Build Phases to ShareExtension Target
        let sourcesPhase = PBXSourcesBuildPhase()
        pbxproj.add(object: sourcesPhase)
        extTarget.buildPhases.append(sourcesPhase)

        if let swiftFileRef = swiftFileRef {
            let buildFile = PBXBuildFile(file: swiftFileRef)
            pbxproj.add(object: buildFile)
            sourcesPhase.files?.append(buildFile)
        }

        let resourcesPhase = PBXResourcesBuildPhase()
        pbxproj.add(object: resourcesPhase)
        extTarget.buildPhases.append(resourcesPhase)

        // 7. Make Runner Target depend on ShareExtension Target
        let proxy = PBXContainerItemProxy(
            containerPortal: .project(rootObject),
            remoteGlobalID: .string(extTarget.uuid),
            proxyType: .nativeTarget,
            remoteInfo: "ShareExtension"
        )
        pbxproj.add(object: proxy)

        let targetDep = PBXTargetDependency(target: extTarget, targetProxy: proxy)
        pbxproj.add(object: targetDep)
        runnerTarget.dependencies.append(targetDep)

        // 8. Embed the App Extension (.appex) inside the Runner App Bundle (Contents/PlugIns)
        // Check if there is already a Copy Files build phase for PlugIns
        var copyPluginsPhase = runnerTarget.buildPhases.compactMap { $0 as? PBXCopyFilesBuildPhase }.first(where: { $0.dstSubfolderSpec == .plugins })
        if copyPluginsPhase == nil {
            copyPluginsPhase = PBXCopyFilesBuildPhase(
                dstPath: "",
                dstSubfolderSpec: .plugins,
                name: "Embed App Extensions"
            )
            pbxproj.add(object: copyPluginsPhase!)
            runnerTarget.buildPhases.append(copyPluginsPhase!)
        }

        let appexBuildFile = PBXBuildFile(file: appexRef)
        pbxproj.add(object: appexBuildFile)
        copyPluginsPhase?.files?.append(appexBuildFile)

        // 9. Save Changes
        try xcodeproj.write(path: projectPath)
        print("Successfully added ShareExtension target and linked it to the Runner target!")

    } catch {
        print("Error: \(error)")
        exit(1)
    }
}

main()
