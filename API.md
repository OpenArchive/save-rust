# Save-Rust API Documentation

This document provides detailed information about the Save-Rust API endpoints, including request/response schemas and error handling.

## Table of Contents
- [General Endpoints](#general-endpoints)
- [Groups Endpoints](#groups-endpoints)
- [Repositories Endpoints](#repositories-endpoints)
- [Media Endpoints](#media-endpoints)

## General Endpoints

### GET /status
Returns the server status and version information.

Response:
```json
{
    "status": "running",
    "version": "string"  // Current version of the server
}
```

Error Response (500 Internal Server Error):
```json
{
    "status": "error",
    "error": "Something went wrong: [detailed error message]"
}
```

### GET /health
Returns the server health status.

Response:
```json
{
    "status": "OK"
}
```

Error Response (500 Internal Server Error):
```json
{
    "status": "error",
    "error": "Something went wrong: [detailed error message]"
}
```

### POST /api/memberships
Joins a group using a membership URL.

Request Body:
```json
{
    "group_url": "string"  // URL containing group information
}
```

Response:
```json
{
    "status_message": "string"  // Success or error message
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group URL: [detailed error message]"
}
```

Error Response (500 Internal Server Error):
```json
{
    "status": "error",
    "error": "Failed to join group: [detailed error message]"
}
```

## Groups Endpoints

Base path: `/api/groups`

### GET /
Lists all groups.

Response:
```json
{
    "groups": [
        {
            "key": "string",     // Base64 encoded group ID
            "name": "string",    // Optional group name
            "created_at": "string" // ISO 8601 timestamp
        }
    ]
}
```

Error Response (500 Internal Server Error):
```json
{
    "status": "error",
    "error": "Failed to list groups: [detailed error message]"
}
```

### POST /
Creates a new group.

Request Body:
```json
{
    "name": "string"  // Name for the new group
}
```

Response:
```json
{
    "key": "string",     // Base64 encoded group ID
    "name": "string",    // Group name
    "created_at": "string" // ISO 8601 timestamp
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group name: [detailed error message]"
}
```

Error Response (500 Internal Server Error):
```json
{
    "status": "error",
    "error": "Failed to create group: [detailed error message]"
}
```

### POST /join_from_url
Joins a group using a URL.

Request Body:
```json
{
    "group_url": "string"  // URL containing group information
}
```

Response:
```json
{
    "key": "string",     // Base64 encoded group ID
    "name": "string",    // Group name
    "created_at": "string" // ISO 8601 timestamp
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group URL: [detailed error message]"
}
```

Error Response (500 Internal Server Error):
```json
{
    "status": "error",
    "error": "Failed to join group: [detailed error message]"
}
```

### GET /{group_id}
Retrieves a specific group by its ID.

Response:
```json
{
    "key": "string",     // Base64 encoded group ID
    "name": "string",    // Group name
    "created_at": "string" // ISO 8601 timestamp
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group not found: [detailed error message]"
}
```

### DELETE /{group_id}
Deletes a group by its ID.

Response:
```json
{
    "status": "success"
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group not found: [detailed error message]"
}
```

### POST /{group_id}/refresh
Refreshes a group by its ID.

Response:
```json
{
    "status": "success",
    "repos": [
        {
            "name": "string",           // Repository name
            "can_write": boolean,       // Whether the user can write to this repo
            "repo_hash": "string",      // Hash of the repository
            "refreshed_files": [        // List of files that were refreshed
                "string"                // File names
            ],
            "all_files": [              // List of all files in the repository
                "string"                // File names
            ]
        }
    ]
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group not found: [detailed error message]"
}
```

## Repositories Endpoints

Base path: `/api/groups/{group_id}/repos`

### GET /
Lists all repositories within a group.

Response:
```json
{
    "repos": [
        {
            "key": "string",     // Base64 encoded repository ID
            "name": "string",    // Repository name
            "created_at": "string" // ISO 8601 timestamp
        }
    ]
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group not found: [detailed error message]"
}
```

### POST /
Creates a new repository within a group.

Request Body:
```json
{
    "name": "string"  // Name for the new repository
}
```

Response:
```json
{
    "key": "string",     // Base64 encoded repository ID
    "name": "string",    // Repository name
    "created_at": "string" // ISO 8601 timestamp
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID or repository name: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group not found: [detailed error message]"
}
```

### GET /{repo_id}
Retrieves a specific repository within a group.

Response:
```json
{
    "key": "string",     // Base64 encoded repository ID
    "name": "string",    // Repository name
    "created_at": "string" // ISO 8601 timestamp
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID or repository ID: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group or repository not found: [detailed error message]"
}
```

## Media Endpoints

Base path: `/api/groups/{group_id}/repos/{repo_id}/media`

### GET /
Lists all files in a repository.

Response:
```json
{
    "files": [
        {
            "name": "string",    // File name
            "size": number,      // File size in bytes
            "created_at": "string" // ISO 8601 timestamp
        }
    ]
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID or repository ID: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group or repository not found: [detailed error message]"
}
```

### POST /{file_name}
Uploads a file to a repository.

Request Body: Binary file content

Response:
```json
{
    "name": "string",                // File name
    "updated_collection_hash": "string" // Hash of the updated collection
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID, repository ID, or file content: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group or repository not found: [detailed error message]"
}
```

Error Response (413 Payload Too Large):
```json
{
    "status": "error",
    "error": "File too large: [detailed error message]"
}
```

### GET /{file_name}
Downloads a specific file from a repository.

Response: Binary file content with appropriate Content-Type header

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID, repository ID, or file name: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group, repository, or file not found: [detailed error message]"
}
```

### DELETE /{file_name}
Deletes a specific file from a repository.

Response:
```json
{
    "collection_hash": "string"  // Hash of the updated collection after deletion
}
```

Error Response (400 Bad Request):
```json
{
    "status": "error",
    "error": "Invalid group ID, repository ID, or file name: [detailed error message]"
}
```

Error Response (404 Not Found):
```json
{
    "status": "error",
    "error": "Group, repository, or file not found: [detailed error message]"
}
``` 