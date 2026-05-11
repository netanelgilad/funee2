use deno_core::{error::AnyError, Extension, FastString, OpDecl, PollEventLoopOptions};

/// Bootstrap JavaScript that sets up timer globals (setTimeout, setInterval, etc.)
const TIMER_BOOTSTRAP: &str = r#"
(() => {
    // Timer callback registry
    const timerCallbacks = new Map();
    
    // setTimeout implementation
    globalThis.setTimeout = (callback, delay = 0, ...args) => {
        const timerId = Deno.core.ops.op_timerStart();
        timerCallbacks.set(timerId, { callback, args, cleared: false });
        
        // Schedule the async wait
        (async () => {
            const completed = await Deno.core.ops.op_timerWait(timerId, delay);
            const timer = timerCallbacks.get(timerId);
            timerCallbacks.delete(timerId);
            if (completed && timer && !timer.cleared) {
                timer.callback(...timer.args);
            }
        })();
        
        return timerId;
    };
    
    // clearTimeout implementation
    globalThis.clearTimeout = (timerId) => {
        const timer = timerCallbacks.get(timerId);
        if (timer) {
            timer.cleared = true;
        }
        Deno.core.ops.op_timerCancel(timerId);
    };
    
    // setInterval implementation
    globalThis.setInterval = (callback, delay = 0, ...args) => {
        let active = true;
        let currentTimerId;
        
        const scheduleNext = async () => {
            while (active) {
                currentTimerId = Deno.core.ops.op_timerStart();
                const completed = await Deno.core.ops.op_timerWait(currentTimerId, delay);
                if (completed && active) {
                    callback(...args);
                } else {
                    break;
                }
            }
        };
        
        // Start the interval loop
        scheduleNext();
        
        // Return an object that tracks the interval
        // We use a unique ID for clearInterval
        const intervalId = Deno.core.ops.op_timerStart();
        timerCallbacks.set(intervalId, { 
            type: 'interval',
            cancel: () => { 
                active = false; 
                if (currentTimerId !== undefined) {
                    Deno.core.ops.op_timerCancel(currentTimerId);
                }
            } 
        });
        
        return intervalId;
    };
    
    // clearInterval implementation
    globalThis.clearInterval = (intervalId) => {
        const interval = timerCallbacks.get(intervalId);
        if (interval && interval.type === 'interval') {
            interval.cancel();
            timerCallbacks.delete(intervalId);
        }
    };
})();
"#;

const TEXT_BOOTSTRAP: &str = r#"
(() => {
    if (globalThis.TextDecoder) {
        return;
    }

    globalThis.TextDecoder = class TextDecoder {
        decode(bytes = new Uint8Array()) {
            let result = "";
            for (let i = 0; i < bytes.length; i++) {
                const b1 = bytes[i];
                if (b1 < 0x80) {
                    result += String.fromCharCode(b1);
                } else if ((b1 & 0xE0) === 0xC0) {
                    const b2 = bytes[++i];
                    result += String.fromCharCode(((b1 & 0x1F) << 6) | (b2 & 0x3F));
                } else if ((b1 & 0xF0) === 0xE0) {
                    const b2 = bytes[++i];
                    const b3 = bytes[++i];
                    result += String.fromCharCode(((b1 & 0x0F) << 12) | ((b2 & 0x3F) << 6) | (b3 & 0x3F));
                } else {
                    const b2 = bytes[++i];
                    const b3 = bytes[++i];
                    const b4 = bytes[++i];
                    let codePoint = ((b1 & 0x07) << 18) | ((b2 & 0x3F) << 12) | ((b3 & 0x3F) << 6) | (b4 & 0x3F);
                    codePoint -= 0x10000;
                    result += String.fromCharCode(0xD800 + (codePoint >> 10), 0xDC00 + (codePoint & 0x3FF));
                }
            }
            return result;
        }
    };
})();
"#;

/// Bootstrap JavaScript that sets up web-standard fetch API (Headers, Response, fetch)
const FETCH_BOOTSTRAP: &str = r#"
(() => {
    // ========================================================================
    // URL class - Web-standard URL implementation
    // ========================================================================
    function URL(url, base) {
        let fullUrl = url;
        
        // Handle base URL
        if (base) {
            // Simple base URL resolution
            if (!url.match(/^[a-z]+:\/\//i)) {
                // Relative URL
                const baseUrl = typeof base === 'string' ? base : base.href;
                if (url.startsWith('/')) {
                    // Absolute path
                    const match = baseUrl.match(/^([a-z]+:\/\/[^\/]+)/i);
                    fullUrl = match ? match[1] + url : url;
                } else {
                    // Relative path
                    const lastSlash = baseUrl.lastIndexOf('/');
                    fullUrl = baseUrl.substring(0, lastSlash + 1) + url;
                }
            }
        }
        
        // Parse URL
        const match = fullUrl.match(/^([a-z]+):\/\/([^:\/]+)(?::(\d+))?(\/[^\?#]*)?(\?[^#]*)?(#.*)?$/i);
        
        if (!match) {
            throw new TypeError(`Invalid URL: ${fullUrl}`);
        }
        
        const [, protocol, hostname, port, pathname, search, hash] = match;
        
        this.href = fullUrl;
        this.protocol = protocol + ':';
        this.hostname = hostname;
        this.port = port || '';
        this.host = port ? `${hostname}:${port}` : hostname;
        this.pathname = pathname || '/';
        this.search = search || '';
        this.hash = hash || '';
        this.origin = `${protocol}://${this.host}`;
        
        // Parse search params
        this.searchParams = new URLSearchParams(this.search);
    }
    
    URL.prototype.toString = function() {
        return this.href;
    };
    
    // ========================================================================
    // URLSearchParams class
    // ========================================================================
    function URLSearchParams(init) {
        const params = new Map();
        
        if (typeof init === 'string') {
            const queryString = init.startsWith('?') ? init.slice(1) : init;
            if (queryString) {
                for (const pair of queryString.split('&')) {
                    const [key, value = ''] = pair.split('=').map(decodeURIComponent);
                    if (!params.has(key)) params.set(key, []);
                    params.get(key).push(value);
                }
            }
        } else if (init instanceof URLSearchParams) {
            for (const [key, value] of init.entries()) {
                if (!params.has(key)) params.set(key, []);
                params.get(key).push(value);
            }
        } else if (typeof init === 'object' && init !== null) {
            for (const [key, value] of Object.entries(init)) {
                params.set(key, [String(value)]);
            }
        }
        
        this.get = (name) => {
            const values = params.get(name);
            return values ? values[0] : null;
        };
        
        this.getAll = (name) => {
            return params.get(name) || [];
        };
        
        this.has = (name) => params.has(name);
        
        this.set = (name, value) => {
            params.set(name, [String(value)]);
        };
        
        this.append = (name, value) => {
            if (!params.has(name)) params.set(name, []);
            params.get(name).push(String(value));
        };
        
        this.delete = (name) => {
            params.delete(name);
        };
        
        this.entries = function* () {
            for (const [key, values] of params) {
                for (const value of values) {
                    yield [key, value];
                }
            }
        };
        
        this.keys = function* () {
            for (const [key] of params) {
                yield key;
            }
        };
        
        this.values = function* () {
            for (const [, values] of params) {
                for (const value of values) {
                    yield value;
                }
            }
        };
        
        this.forEach = (callback) => {
            for (const [key, value] of this.entries()) {
                callback(value, key, this);
            }
        };
        
        this.toString = () => {
            const pairs = [];
            for (const [key, value] of this.entries()) {
                pairs.push(`${encodeURIComponent(key)}=${encodeURIComponent(value)}`);
            }
            return pairs.join('&');
        };
        
        this[Symbol.iterator] = this.entries;
    }
    
    globalThis.URL = URL;
    globalThis.URLSearchParams = URLSearchParams;
    
    // ========================================================================
    // Headers class - Web-standard Headers implementation
    // ========================================================================
    function Headers(init) {
        // Private storage - Map of lowercase name -> array of values
        const _headers = new Map();
        
        // Helper: normalize header name (lowercase)
        const normalizeName = (name) => {
            if (typeof name !== 'string') throw new TypeError('Header name must be a string');
            return name.toLowerCase();
        };
        
        // Helper: validate header value
        const normalizeValue = (value) => {
            if (typeof value !== 'string') return String(value);
            return value;
        };
        
        // Initialize from various input types
        if (init) {
            if (Array.isArray(init)) {
                // Array of [name, value] pairs - check BEFORE Headers check
                // since arrays also have entries() method
                for (const [name, value] of init) {
                    const key = normalizeName(name);
                    if (!_headers.has(key)) _headers.set(key, []);
                    _headers.get(key).push(normalizeValue(value));
                }
            } else if (init instanceof Headers || (init && typeof init.get === 'function' && typeof init.set === 'function')) {
                // Copy from another Headers or Headers-like object
                // Check for get/set methods instead of entries() to avoid matching arrays
                for (const [name, value] of init.entries()) {
                    _headers.set(normalizeName(name), [normalizeValue(value)]);
                }
            } else if (typeof init === 'object') {
                // Plain object
                for (const [name, value] of Object.entries(init)) {
                    _headers.set(normalizeName(name), [normalizeValue(value)]);
                }
            }
        }
        
        // get(name) - returns combined value or null
        this.get = (name) => {
            const values = _headers.get(normalizeName(name));
            return values ? values.join(', ') : null;
        };
        
        // set(name, value) - replaces all values
        this.set = (name, value) => {
            _headers.set(normalizeName(name), [normalizeValue(value)]);
        };
        
        // has(name) - check if header exists
        this.has = (name) => {
            return _headers.has(normalizeName(name));
        };
        
        // delete(name) - remove header
        this.delete = (name) => {
            _headers.delete(normalizeName(name));
        };
        
        // append(name, value) - add value (combines with existing)
        this.append = (name, value) => {
            const key = normalizeName(name);
            if (!_headers.has(key)) {
                _headers.set(key, []);
            }
            _headers.get(key).push(normalizeValue(value));
        };
        
        // entries() - iterator of [name, value] pairs
        this.entries = function* () {
            for (const [name, values] of _headers) {
                yield [name, values.join(', ')];
            }
        };
        
        // keys() - iterator of header names
        this.keys = function* () {
            for (const name of _headers.keys()) {
                yield name;
            }
        };
        
        // values() - iterator of header values
        this.values = function* () {
            for (const values of _headers.values()) {
                yield values.join(', ');
            }
        };
        
        // forEach(callback)
        this.forEach = (callback) => {
            for (const [name, values] of _headers) {
                callback(values.join(', '), name, this);
            }
        };
        
        // Make iterable (same as entries)
        this[Symbol.iterator] = this.entries;
    }
    
    // ========================================================================
    // Response class - Web-standard Response implementation
    // ========================================================================
    function Response(body, init) {
        const _init = init || {};
        const _status = _init.status !== undefined ? _init.status : 200;
        const _statusText = _init.statusText !== undefined ? _init.statusText : '';
        const _headers = _init.headers instanceof Headers 
            ? _init.headers 
            : new Headers(_init.headers);
        const _url = _init.url || '';
        const _redirected = _init.redirected || false;
        
        let _body = body !== undefined && body !== null ? String(body) : null;
        let _bodyUsed = false;
        
        // Read-only properties
        Object.defineProperties(this, {
            ok: { get: () => _status >= 200 && _status < 300, enumerable: true },
            status: { get: () => _status, enumerable: true },
            statusText: { get: () => _statusText, enumerable: true },
            headers: { get: () => _headers, enumerable: true },
            url: { get: () => _url, enumerable: true },
            redirected: { get: () => _redirected, enumerable: true },
            type: { get: () => 'basic', enumerable: true },
            bodyUsed: { get: () => _bodyUsed, enumerable: true }
        });
        
        // Helper to consume body
        const consumeBody = () => {
            if (_bodyUsed) {
                throw new TypeError('Body has already been consumed');
            }
            _bodyUsed = true;
            return _body;
        };
        
        // text() - get body as string
        this.text = async () => {
            const body = consumeBody();
            return body || '';
        };
        
        // json() - parse body as JSON
        this.json = async () => {
            const body = consumeBody();
            if (!body) throw new SyntaxError('Unexpected end of JSON input');
            return JSON.parse(body);
        };
        
        // arrayBuffer() - get body as ArrayBuffer
        this.arrayBuffer = async () => {
            const body = consumeBody();
            const encoder = new TextEncoder();
            return encoder.encode(body || '').buffer;
        };
        
        // bytes() - get body as Uint8Array
        this.bytes = async () => {
            const body = consumeBody();
            const encoder = new TextEncoder();
            return encoder.encode(body || '');
        };
        
        // blob() - get body as Blob-like object
        this.blob = async () => {
            const body = consumeBody();
            const data = new TextEncoder().encode(body || '');
            return {
                size: data.length,
                type: _headers.get('content-type') || '',
                arrayBuffer: async () => data.buffer,
                text: async () => body || '',
            };
        };
        
        // clone() - create a copy
        this.clone = () => {
            if (_bodyUsed) {
                throw new TypeError('Cannot clone a Response whose body has been consumed');
            }
            return new Response(_body, {
                status: _status,
                statusText: _statusText,
                headers: new Headers(_headers),
                url: _url,
                redirected: _redirected
            });
        };
    }
    
    // Static methods on Response
    Response.error = () => {
        const r = new Response(null, { status: 0, statusText: '' });
        Object.defineProperty(r, 'type', { get: () => 'error' });
        return r;
    };
    
    Response.redirect = (url, status = 302) => {
        if (![301, 302, 303, 307, 308].includes(status)) {
            throw new RangeError('Invalid redirect status code');
        }
        return new Response(null, {
            status,
            headers: { 'Location': url }
        });
    };
    
    Response.json = (data, init) => {
        const body = JSON.stringify(data);
        const headers = new Headers(init?.headers);
        if (!headers.has('content-type')) {
            headers.set('content-type', 'application/json');
        }
        return new Response(body, {
            status: init?.status || 200,
            statusText: init?.statusText || '',
            headers
        });
    };
    
    // ========================================================================
    // fetch() - Web-standard fetch implementation
    // ========================================================================
    async function fetch(input, init) {
        // Normalize input to URL string
        let url;
        if (typeof input === 'string') {
            url = input;
        } else if (input && typeof input.url === 'string') {
            // Request-like object
            url = input.url;
            // Merge init from Request if not provided
            if (!init) {
                init = {
                    method: input.method,
                    headers: input.headers
                };
            }
        } else if (input && input.toString) {
            // URL object
            url = input.toString();
        } else {
            throw new TypeError('Invalid fetch input');
        }
        
        const options = init || {};
        const method = options.method || 'GET';
        const body = options.body || '';
        const followRedirects = options.redirect !== 'error' && options.redirect !== 'manual';
        
        // Build headers JSON
        let headersObj = {};
        if (options.headers) {
            if (options.headers instanceof Headers) {
                for (const [name, value] of options.headers.entries()) {
                    headersObj[name] = value;
                }
            } else if (Array.isArray(options.headers)) {
                for (const [name, value] of options.headers) {
                    headersObj[name] = value;
                }
            } else {
                headersObj = options.headers;
            }
        }
        const headersJson = JSON.stringify(headersObj);
        
        // Call the Rust op
        const resultJson = await Deno.core.ops.op_fetch(
            method,
            url,
            headersJson,
            body,
            followRedirects
        );
        
        // Parse result
        const result = JSON.parse(resultJson);
        
        // Handle redirect: "error" - should have thrown if redirect happened with followRedirects=false
        if (options.redirect === 'error' && result.redirected) {
            throw new TypeError('Redirect not allowed');
        }
        
        // Handle redirect: "manual" - return a special opaque redirect response
        if (options.redirect === 'manual' && result.status >= 300 && result.status < 400) {
            // For manual redirect, return the redirect response as-is
        }
        
        // Build Response object
        return new Response(result.body, {
            status: result.status,
            statusText: result.statusText,
            headers: result.headers,
            url: result.url,
            redirected: result.redirected
        });
    }
    
    // Expose globals
    globalThis.Headers = Headers;
    globalThis.Response = Response;
    globalThis.fetch = fetch;
})();
"#;

/// Bootstrap JavaScript that sets up HTTP server (serve function)
const SERVER_BOOTSTRAP: &str = r#"
(() => {
    // ========================================================================
    // serve() - Deno-style HTTP server
    // ========================================================================
    
    /**
     * Create server-side Request from raw request info
     */
    function createServerRequest(raw, port) {
        // raw.url already contains the path+query, e.g. "/test?foo=bar"
        const fullUrl = `http://127.0.0.1:${port}${raw.url}`;
        
        // Build headers
        const headers = new Headers();
        for (const [name, value] of raw.headers) {
            headers.set(name, value);
        }
        
        // Body reading state
        let bodyRead = false;
        let cachedBody = null;
        
        const getBody = () => {
            if (!bodyRead) {
                cachedBody = Deno.core.ops.op_serverReadBody(raw.request_id);
                bodyRead = true;
            }
            return cachedBody || "";
        };
        
        // Create Request-like object
        return {
            method: raw.method,
            url: fullUrl,
            headers,
            body: raw.has_body ? {} : null,
            bodyUsed: false,
            
            async text() {
                if (this.bodyUsed) {
                    throw new TypeError("Body has already been consumed");
                }
                this.bodyUsed = true;
                return getBody();
            },
            
            async json() {
                const text = await this.text();
                return JSON.parse(text);
            },
            
            async arrayBuffer() {
                const text = await this.text();
                const encoder = new TextEncoder();
                return encoder.encode(text).buffer;
            }
        };
    }
    
    /**
     * Send response to client
     */
    async function sendResponse(serverId, requestId, response) {
        // Extract headers
        const headersObj = {};
        for (const [name, value] of response.headers.entries()) {
            headersObj[name] = value;
        }
        
        // Get body (may have already been read for cloned responses)
        let body = "";
        if (!response.bodyUsed) {
            try {
                body = await response.text();
            } catch (e) {
                // Body already consumed or not available
            }
        }
        
        await Deno.core.ops.op_serverRespond(
            serverId,
            requestId,
            response.status,
            JSON.stringify(headersObj),
            body
        );
    }
    
    /**
     * Handle a single request
     */
    async function handleRequest(serverId, port, raw, handler, onError) {
        try {
            const request = createServerRequest(raw, port);
            const response = await handler(request);
            await sendResponse(serverId, raw.request_id, response);
        } catch (error) {
            let errorResponse;
            if (onError) {
                try {
                    errorResponse = await onError(error);
                } catch {
                    errorResponse = new Response("Internal Server Error", { status: 500 });
                }
            } else {
                errorResponse = new Response("Internal Server Error", { status: 500 });
            }
            await sendResponse(serverId, raw.request_id, errorResponse);
        }
    }
    
    /**
     * Create an HTTP server
     * 
     * The server starts listening synchronously so port is available immediately.
     */
    globalThis.serve = (options, handler) => {
        const port = options.port;
        const hostname = options.hostname || "127.0.0.1";
        const onListen = options.onListen;
        const onError = options.onError;
        
        let isShuttingDown = false;
        let pendingRequests = 0;
        let shutdownResolve = null;
        
        // Start server synchronously - port is available immediately
        const resultJson = Deno.core.ops.op_serverStart(port, hostname);
        const result = JSON.parse(resultJson);
        const serverId = result.server_id;
        const actualPort = result.port;
        const actualHostname = result.hostname;
        
        // Call onListen
        if (onListen) {
            onListen({ port: actualPort, hostname: actualHostname });
        }
        
        // Wrapper to track pending requests
        const handleRequestWithTracking = async (serverId, port, raw, handler, onError) => {
            pendingRequests++;
            try {
                await handleRequest(serverId, port, raw, handler, onError);
            } finally {
                pendingRequests--;
                // Check if we're shutting down and all requests are done
                if (isShuttingDown && pendingRequests === 0 && shutdownResolve) {
                    shutdownResolve();
                }
            }
        };
        
        // Track if accept loop is waiting
        let acceptLoopStopped = false;
        let acceptLoopResolve = null;
        
        // Start accept loop asynchronously
        const acceptLoop = async () => {
            while (!isShuttingDown) {
                const requestJson = await Deno.core.ops.op_serverAccept(serverId);
                
                if (requestJson === null || requestJson === "null" || isShuttingDown) {
                    break;
                }
                
                const raw = JSON.parse(requestJson);
                
                // Handle request concurrently (don't await)
                handleRequestWithTracking(serverId, actualPort, raw, handler, onError).catch(() => {});
            }
            acceptLoopStopped = true;
            if (acceptLoopResolve) acceptLoopResolve();
        };
        
        // Start accept loop (don't await - runs in background)
        acceptLoop().catch(() => {});
        
        // Shutdown function
        const shutdown = async () => {
            isShuttingDown = true;
            
            // Wait for pending requests to complete
            if (pendingRequests > 0) {
                await new Promise(resolve => {
                    let timeoutId = null;
                    shutdownResolve = () => {
                        if (timeoutId !== null) {
                            clearTimeout(timeoutId);
                        }
                        resolve();
                    };
                    // Also set a timeout just in case
                    timeoutId = setTimeout(shutdownResolve, 30000);
                });
            }
            
            await Deno.core.ops.op_serverStop(serverId);
        };
        
        // Return server handle
        return {
            get port() {
                return actualPort;
            },
            get hostname() {
                return actualHostname;
            },
            shutdown,
            [Symbol.asyncDispose]: shutdown
        };
    };
})();
"#;

const SUBPROCESS_BOOTSTRAP: &str = r#"
(() => {
    // Base64 encode/decode helpers (without using btoa/atob which aren't available)
    const base64Chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    
    const base64Encode = (data) => {
        const bytes = typeof data === "string" 
            ? utf8Encode(data)
            : data;
        
        let result = "";
        let i = 0;
        while (i < bytes.length) {
            const b1 = bytes[i++];
            const hasB2 = i < bytes.length;
            const b2 = hasB2 ? bytes[i++] : 0;
            const hasB3 = i < bytes.length;
            const b3 = hasB3 ? bytes[i++] : 0;
            
            result += base64Chars.charAt(b1 >> 2);
            result += base64Chars.charAt(((b1 & 3) << 4) | (b2 >> 4));
            result += hasB2 ? base64Chars.charAt(((b2 & 15) << 2) | (b3 >> 6)) : "=";
            result += hasB3 ? base64Chars.charAt(b3 & 63) : "=";
        }
        return result;
    };

    const base64Decode = (encoded) => {
        const lookup = {};
        for (let i = 0; i < base64Chars.length; i++) {
            lookup[base64Chars.charAt(i)] = i;
        }
        
        const len = encoded.length;
        let bufferLength = Math.floor(len * 0.75);
        
        // Adjust for padding
        let padding = 0;
        if (encoded[len - 1] === "=") padding++;
        if (encoded[len - 2] === "=") padding++;
        bufferLength -= padding;
        
        const bytes = new Uint8Array(bufferLength);
        let p = 0;
        
        for (let i = 0; i < len; i += 4) {
            const c1 = lookup[encoded.charAt(i)];
            const c2 = lookup[encoded.charAt(i + 1)];
            const c3 = encoded.charAt(i + 2) === "=" ? undefined : lookup[encoded.charAt(i + 2)];
            const c4 = encoded.charAt(i + 3) === "=" ? undefined : lookup[encoded.charAt(i + 3)];
            
            if (p < bufferLength) bytes[p++] = (c1 << 2) | (c2 >> 4);
            if (c3 !== undefined && p < bufferLength) bytes[p++] = ((c2 & 15) << 4) | (c3 >> 2);
            if (c4 !== undefined && p < bufferLength) bytes[p++] = ((c3 & 3) << 6) | c4;
        }
        
        return bytes;
    };

    // Simple UTF-8 decoder
    const utf8Decode = (bytes) => {
        let result = "";
        let i = 0;
        while (i < bytes.length) {
            const byte1 = bytes[i++];
            if (byte1 < 0x80) {
                result += String.fromCharCode(byte1);
            } else if (byte1 < 0xE0) {
                const byte2 = bytes[i++];
                result += String.fromCharCode(((byte1 & 0x1F) << 6) | (byte2 & 0x3F));
            } else if (byte1 < 0xF0) {
                const byte2 = bytes[i++];
                const byte3 = bytes[i++];
                result += String.fromCharCode(((byte1 & 0x0F) << 12) | ((byte2 & 0x3F) << 6) | (byte3 & 0x3F));
            } else {
                const byte2 = bytes[i++];
                const byte3 = bytes[i++];
                const byte4 = bytes[i++];
                let codePoint = ((byte1 & 0x07) << 18) | ((byte2 & 0x3F) << 12) | ((byte3 & 0x3F) << 6) | (byte4 & 0x3F);
                codePoint -= 0x10000;
                result += String.fromCharCode(0xD800 + (codePoint >> 10));
                result += String.fromCharCode(0xDC00 + (codePoint & 0x3FF));
            }
        }
        return result;
    };

    // Simple UTF-8 encoder  
    const utf8Encode = (str) => {
        const bytes = [];
        for (let i = 0; i < str.length; i++) {
            let code = str.charCodeAt(i);
            
            // Handle surrogate pairs
            if (code >= 0xD800 && code <= 0xDBFF && i + 1 < str.length) {
                const next = str.charCodeAt(i + 1);
                if (next >= 0xDC00 && next <= 0xDFFF) {
                    code = 0x10000 + ((code & 0x3FF) << 10) + (next & 0x3FF);
                    i++;
                }
            }
            
            if (code < 0x80) {
                bytes.push(code);
            } else if (code < 0x800) {
                bytes.push(0xC0 | (code >> 6));
                bytes.push(0x80 | (code & 0x3F));
            } else if (code < 0x10000) {
                bytes.push(0xE0 | (code >> 12));
                bytes.push(0x80 | ((code >> 6) & 0x3F));
                bytes.push(0x80 | (code & 0x3F));
            } else {
                bytes.push(0xF0 | (code >> 18));
                bytes.push(0x80 | ((code >> 12) & 0x3F));
                bytes.push(0x80 | ((code >> 6) & 0x3F));
                bytes.push(0x80 | (code & 0x3F));
            }
        }
        return new Uint8Array(bytes);
    };

    // Spawn function that returns a Process handle or Promise<CommandOutput>
    globalThis.spawn = (commandOrOptions, args) => {
        const ops = Deno.core.ops;
        
        // Normalize to options object
        const isSimpleForm = typeof commandOrOptions === "string";
        const options = isSimpleForm
            ? { 
                cmd: [commandOrOptions, ...(args || [])],
                stdout: "piped",
                stderr: "piped",
                stdin: "null",
              }
            : {
                stdout: commandOrOptions.stdout || "piped",
                stderr: commandOrOptions.stderr || "piped",
                stdin: commandOrOptions.stdin || "null",
                ...commandOrOptions,
              };
        
        // Spawn the process
        const resultJson = ops.op_processSpawn(
            JSON.stringify(options.cmd),
            options.cwd || "",
            JSON.stringify(options.env || {}),
            options.inheritEnv !== false,
            options.stdin || "null",
            options.stdout || "piped",
            options.stderr || "piped",
        );
        
        const result = JSON.parse(resultJson);
        const processId = result.process_id;
        const pid = result.pid;

        const stdoutBase64Promise = options.stdout === "piped"
            ? ops.op_processReadStdout(processId)
            : Promise.resolve("");
        const stderrBase64Promise = options.stderr === "piped"
            ? ops.op_processReadStderr(processId)
            : Promise.resolve("");
        
        let stdinClosed = false;
        let statusPromise = null;
        
        const getStatus = () => {
            if (!statusPromise) {
                statusPromise = (async () => {
                    const waitResultJson = await ops.op_processWait(processId);
                    const waitResult = JSON.parse(waitResultJson);
                    return {
                        success: waitResult.success,
                        code: waitResult.code,
                        signal: waitResult.signal,
                    };
                })();
            }
            return statusPromise;
        };
        
        const process = {
            pid,
            
            get status() {
                return getStatus();
            },
            
            kill(signal = "SIGTERM") {
                ops.op_processKill(processId, signal);
            },

            async completion() {
                const status = await getStatus();
                return {
                    status: status.code ?? 0,
                    signal: status.signal,
                };
            },
            
            async output() {
                const [stdoutBase64, stderrBase64] = await Promise.all([stdoutBase64Promise, stderrBase64Promise]);
                
                const status = await getStatus();
                
                const stdout = stdoutBase64 ? base64Decode(stdoutBase64) : new Uint8Array(0);
                const stderr = stderrBase64 ? base64Decode(stderrBase64) : new Uint8Array(0);
                
                return {
                    status,
                    stdout,
                    stderr,
                    stdoutText: () => utf8Decode(stdout),
                    stderrText: () => utf8Decode(stderr),
                };
            },
            
            async writeInput(data) {
                if (options.stdin !== "piped") {
                    throw new Error("Cannot write to stdin: stdin is not piped");
                }
                if (stdinClosed) {
                    throw new Error("Cannot write to stdin: stdin already closed");
                }
                
                const encoded = base64Encode(data);
                await ops.op_processWrite(processId, encoded);
                
                ops.op_processCloseStdin(processId);
                stdinClosed = true;
            },

            async [Symbol.asyncDispose]() {
                try {
                    ops.op_processKill(processId, "SIGTERM");
                } catch {
                    // Process may already be exited.
                }
                await getStatus();
            },
        };

        process.stdout = {
            async *[Symbol.asyncIterator]() {
                const stdoutBase64 = await stdoutBase64Promise;
                const stdout = stdoutBase64 ? base64Decode(stdoutBase64) : new Uint8Array(0);
                if (stdout.length > 0) {
                    yield stdout;
                }
            }
        };

        process.stderr = {
            async *[Symbol.asyncIterator]() {
                const stderrBase64 = await stderrBase64Promise;
                const stderr = stderrBase64 ? base64Decode(stderrBase64) : new Uint8Array(0);
                if (stderr.length > 0) {
                    yield stderr;
                }
            }
        };
        
        // For simple form, return Promise<CommandOutput>
        if (isSimpleForm) {
            return process.output();
        }
        
        // For options form, return Process handle
        return process;
    };

    globalThis.env = (name) => {
        const ops = Deno.core.ops;
        const value = ops.op_processEnv(name);
        return value === "" ? undefined : value;
    };
})();
"#;

pub async fn run_js(js: &str, ops: Vec<OpDecl>) -> Result<(), AnyError> {
    let ext = Extension {
        ops: std::borrow::Cow::Owned(ops),
        ..Default::default()
    };
    
    let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
        extensions: vec![ext],
        ..Default::default()
    });

    // Execute timer bootstrap first to set up setTimeout/setInterval globals
    js_runtime.execute_script("[funee:timers.js]", TIMER_BOOTSTRAP)?;

    js_runtime.execute_script("[funee:text.js]", TEXT_BOOTSTRAP)?;
    
    // Execute fetch bootstrap to set up fetch/Headers/Response globals
    js_runtime.execute_script("[funee:fetch.js]", FETCH_BOOTSTRAP)?;
    
    // Execute server bootstrap to set up serve() function
    js_runtime.execute_script("[funee:server.js]", SERVER_BOOTSTRAP)?;
    
    // Execute subprocess bootstrap to set up spawn() function
    js_runtime.execute_script("[funee:subprocess.js]", SUBPROCESS_BOOTSTRAP)?;
    
    // Then execute user code
    let js_code: FastString = js.to_string().into();
    js_runtime.execute_script("[funee:runtime.js]", js_code)?;
    js_runtime.run_event_loop(PollEventLoopOptions::default()).await?;

    Ok(())
}
