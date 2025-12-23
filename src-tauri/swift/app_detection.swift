import AppKit

// MARK: - Get Frontmost Application

/// Get the currently focused application's bundle ID
/// Returns a C string that must be freed with free_string()
@_cdecl("get_frontmost_app_bundle_id")
public func getFrontmostAppBundleId() -> UnsafeMutablePointer<CChar>? {
    guard let app = NSWorkspace.shared.frontmostApplication,
          let bundleId = app.bundleIdentifier else {
        return nil
    }
    return strdup(bundleId)
}

/// Get the currently focused application's display name
/// Returns a C string that must be freed with free_string()
@_cdecl("get_frontmost_app_name")
public func getFrontmostAppName() -> UnsafeMutablePointer<CChar>? {
    guard let app = NSWorkspace.shared.frontmostApplication,
          let name = app.localizedName else {
        return nil
    }
    return strdup(name)
}

/// Free a string allocated by the other functions
@_cdecl("free_string")
public func freeString(_ ptr: UnsafeMutablePointer<CChar>?) {
    if let ptr = ptr {
        free(ptr)
    }
}

// MARK: - Get Installed Applications

/// Get a JSON array of installed applications
/// Returns a JSON string like: [{"bundle_id": "com.example.App", "name": "Example App"}, ...]
/// Must be freed with free_string()
@_cdecl("get_installed_applications_json")
public func getInstalledApplicationsJson() -> UnsafeMutablePointer<CChar>? {
    var apps: [[String: String]] = []
    
    let appDirs = [
        "/Applications",
        "/System/Applications",
        NSHomeDirectory() + "/Applications"
    ]
    
    for dir in appDirs {
        guard let contents = try? FileManager.default.contentsOfDirectory(atPath: dir) else {
            continue
        }
        
        for item in contents where item.hasSuffix(".app") {
            let appPath = "\(dir)/\(item)"
            guard let bundle = Bundle(path: appPath),
                  let bundleId = bundle.bundleIdentifier else {
                continue
            }
            
            // Try to get name from various Info.plist keys
            let name = bundle.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String
                ?? bundle.object(forInfoDictionaryKey: "CFBundleName") as? String
                ?? item.replacingOccurrences(of: ".app", with: "")
            
            apps.append(["bundle_id": bundleId, "name": name])
        }
    }
    
    // Sort by name for consistent ordering
    apps.sort { ($0["name"] ?? "") < ($1["name"] ?? "") }
    
    guard let jsonData = try? JSONSerialization.data(withJSONObject: apps),
          let jsonString = String(data: jsonData, encoding: .utf8) else {
        return strdup("[]")
    }
    
    return strdup(jsonString)
}
