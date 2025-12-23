#ifndef app_detection_bridge_h
#define app_detection_bridge_h

// C-compatible function declarations for Swift app detection bridge

#ifdef __cplusplus
extern "C" {
#endif

// Get the currently focused application's bundle ID
// Returns NULL if not available
// Caller must free with free_string()
char *get_frontmost_app_bundle_id(void);

// Get the currently focused application's display name
// Returns NULL if not available
// Caller must free with free_string()
char *get_frontmost_app_name(void);

// Free a string allocated by the other functions
void free_string(char *ptr);

// Get installed applications as JSON string
// Returns: [{"bundle_id": "...", "name": "..."}, ...]
// Caller must free with free_string()
char *get_installed_applications_json(void);

#ifdef __cplusplus
}
#endif

#endif /* app_detection_bridge_h */
