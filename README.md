# Save-Rust

Bindings to the save-dweb-backend for the Save Android app.

## Requirements

- Install the Android SDK and Android Platform 34 
- Install android NDK on your system
- Install Rust
- `cargo install cargo-ndk`
- Have `save-android` set up in the parent folder
- Set up the `ANDROID_NDK_HOME` variable
- `./build-android.sh`
- You can now recompile the android app.

# API Documentation

The Save-Rust API provides HTTP endpoints for managing groups, repositories, and media files. For detailed API documentation including request/response schemas and error handling, please see [API.md](API.md).

## Available Endpoints

### General
*   `GET /status` - Returns the server status and version.
*   `GET /health` - Returns the server health status.
*   `POST /api/memberships` - Joins a group.

### Groups
Base path: `/api/groups`
*   `GET /` - Lists all groups.
*   `POST /` - Creates a new group.
*   `POST /join_from_url` - Joins a group using a URL.
*   `GET /{group_id}` - Retrieves a specific group by its ID.
*   `DELETE /{group_id}` - Deletes a group by its ID.
*   `POST /{group_id}/refresh` - Refreshes a group by its ID.

### Repositories
Base path: `/api/groups/{group_id}/repos`
*   `GET /` - Lists all repositories within a group.
*   `POST /` - Creates a new repository within a group.
*   `GET /{repo_id}` - Retrieves a specific repository within a group.

### Media
Base path: `/api/groups/{group_id}/repos/{repo_id}/media`
*   `GET /` - Lists all files in a repository.
*   `POST /{file_name}` - Uploads a file to a repository.
*   `GET /{file_name}` - Downloads a specific file from a repository.
*   `DELETE /{file_name}` - Deletes a specific file from a repository.

For detailed information about request/response formats, error handling, and examples, please refer to the [API Documentation](API.md).