/**
 * Host Filesystem Module
 * 
 * Provides file system operations.
 * Import from "host://fs"
 * 
 * Note: Functions return JSON strings with result format:
 * { type: "ok", value: ... } or { type: "error", error: "..." }
 */

/**
 * File/directory stats returned by lstat()
 */
export interface FileStats {
  /** Size in bytes */
  size: number;
  /** True if this is a regular file */
  is_file: boolean;
  /** True if this is a directory */
  is_directory: boolean;
  /** Last modification time as milliseconds since epoch */
  modified_ms: number;
}

/**
 * Read a file as UTF-8 text
 * @returns JSON string with result format
 */
export declare function readFile(path: string): string;

/**
 * Read a file as binary data (base64 encoded)
 * @returns JSON string with result format (value is base64 string)
 */
export declare function readFileBinary(path: string): string;

/**
 * Write text content to a file
 * @returns JSON string with result format
 */
export declare function writeFile(path: string, content: string): string;

/**
 * Remove a file.
 * @returns JSON string with result format
 */
export declare function removeFile(path: string): string;

/**
 * Write binary data to a file (base64 encoded)
 * @returns JSON string with result format
 */
export declare function writeFileBinary(path: string, contentBase64: string): string;

/**
 * Check if path is a regular file
 */
export declare function isFile(path: string): boolean;

/**
 * Check if path exists (file or directory)
 */
export declare function exists(path: string): boolean;

/**
 * Get file/directory stats (does not follow symlinks)
 * @returns FileStats object (automatically parsed from JSON)
 */
export declare function lstat(path: string): string;

/**
 * Create a directory
 */
export declare function mkdir(path: string, recursive?: boolean): void;

/**
 * Read directory contents
 * @returns JSON string with array of entry names
 */
export declare function readdir(path: string): string;

/**
 * Get the system temporary directory path
 */
export declare function tmpdir(): string;
