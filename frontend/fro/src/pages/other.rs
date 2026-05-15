use crate::pages::hardware::components::InfoIcon;
use crate::pages::hardware::constants::{PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE};
use crate::{
    api,
    app::Route,
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
    pages::hardware::components::info_modal,
};
use dioxus::prelude::*;

const LABEL: &str = "text-xs text-gray-400 font-mono whitespace-nowrap";
const INPUT_SM: &str = "input input-xs input-bordered bg-gray-700 text-gray-200 w-24";
const INPUT_MD: &str = "input input-xs input-bordered bg-gray-700 text-gray-200 w-32";
const INPUT_LG: &str = "input input-xs input-bordered bg-gray-700 text-gray-200 w-56";

const OS_THREAD_LINKS: &[(&str, &str)] = &[
    ("Concept",             "Top-level overview of what an OS thread is and how it fits into the operating system's execution model."),
    ("Definition",          "An OS thread is the kernel's fundamental schedulable execution unit with its own stack, register state, and scheduling metadata."),
    ("Execution context",   "Each thread has its own program counter, stack pointer, CPU registers, TLS, and kernel-managed state."),
    ("Stack",               "Every OS thread has a dedicated stack (commonly 512 KB–8 MB) allocated in the process's address space."),
    ("Registers",           "The kernel saves and restores the thread's CPU registers during context switches."),
    ("Scheduling",          "The kernel scheduler decides when a thread runs, pauses, or migrates across CPU cores."),
    ("Preemption",          "Threads are preemptively scheduled: the kernel can interrupt a running thread to run another."),
    ("Context switch",      "A context switch saves the current thread's state and loads another's, enabling multitasking."),
    ("Thread states",       "Typical states include New, Runnable, Running, Blocked/Waiting, and Terminated."),
    ("Relation to process", "A process contains one or more threads; threads share memory, file descriptors, and resources."),
    ("Cost",                "Threads are lighter than processes but still expensive due to stack memory and context-switch overhead."),
    ("Parallelism",         "Multiple OS threads can run truly in parallel on multi-core CPUs."),
    ("Blocking behavior",   "When a thread blocks on I/O or a mutex, the scheduler switches to another runnable thread."),
    ("Scalability limits",  "OS threads scale to thousands, but millions are impractical due to memory and scheduling overhead."),
    ("vs async tasks",      "Async tasks are user-space scheduled, cheaper, and can number in the millions; OS threads cannot."),
    ("Use cases",           "Used for CPU-bound work, blocking operations, and true parallelism across cores."),
];

const DOWNSTREAM_ROUNDTRIP_CONTENT: &str = "A downstream roundtrip means your worker thread sends a request to something outside your server — a database, vector store, microservice, or external API — and must wait for the response before it can continue.\n\nIt is \"downstream\" because the dependency sits below your service in the call chain. It is a \"roundtrip\" because it involves:\n• sending a request out\n• waiting for the remote system\n• receiving a response back\n\nDuring this wait the worker cannot make progress on that request.\n\n🔍 What counts as a downstream roundtrip\n\n• Database query (Postgres, MySQL, SQLite)\n• Vector DB lookup (Qdrant, Milvus, Pinecone)\n• Cache request (Redis, Memcached)\n• HTTP call to another service\n• Cloud API call (S3, OpenAI, Azure)\n• Filesystem I/O (slow disk, network filesystem)\n\nAll of these require waiting for something outside your process.\n\n🧠 Why more workers do not help\n\nWorkers help when the bottleneck is CPU. A downstream roundtrip is I/O latency — not CPU work.\n\nIf the bottleneck is network latency, database latency, remote service latency, lock contention inside the database, or queueing inside a downstream service — adding more workers just creates more threads waiting. The limiting factor is the remote system, not your server.\n\n⚙️ What actually happens during the wait\n\n1. The handler sends a request to the downstream system.\n2. The async runtime yields (await).\n3. The worker thread becomes free to run other tasks.\n4. But if all workers are waiting on downstream I/O, the server is effectively idle.\n5. Adding more workers only increases the number of waiting threads, not throughput.\n\nThis is why downstream latency dominates.\n\n📌 Summary\n\nA downstream roundtrip is a request from your server to an external dependency that requires waiting for a response and is limited by network or remote-system latency — not by your CPU or number of workers.\n\n• CPU-bound → more workers help\n• Downstream-bound → more workers do nothing";

const SEMAPHORE_ETYMOLOGY: &[(&str, &str)] = &[
    ("Semaphore",          "From Greek \u{201c}s\u{ea}ma\u{201d} meaning \u{201c}sign\u{201d} or \u{201c}signal\u{201d}"),
    ("-phoros",            "From Greek \u{201c}-phoros\u{201d} meaning \u{201c}bearing\u{201d} or \u{201c}carrying\u{201d}"),
    ("Literal meaning",    "Literally \u{201c}signal-bearer\u{201d} or \u{201c}one who carries a signal\u{201d}"),
    ("French transition",  "Entered French as \u{201c}s\u{e9}maphore\u{201d} referring to optical telegraph towers with moving arms"),
    ("19th century usage", "Adopted in English for railway signal arms controlling STOP/GO"),
    ("Computing adoption", "Adopted by Dijkstra (1965) for a concurrency primitive regulating access via permits. (Dutch: Seinpaal)"),
    ("Conceptual mapping", "Physical semaphore arms map to software permits: raised=proceed, lowered=wait"),
    ("Continuity",         "Term preserved because it consistently meant a mechanism that controls flow using signals"),
];

const LOGICAL_CPU_CONTENT: &str ="A logical CPU is a scheduling slot the operating system exposes as an independent unit of execution. The OS can assign one thread to each logical CPU simultaneously. It is the number returned by nproc, available_parallelism(), and what appears as distinct CPU entries in /proc/cpuinfo.\n\n🏗️ Where logical CPUs come from\n\nModern hardware has three layers:\n• Socket — the physical chip\n• Physical core — the silicon execution unit with its own ALUs, caches, and registers\n• Hardware thread — a separate register file and instruction frontend per SMT slot\n\nA physical core executes instructions. With SMT (Simultaneous Multi-Threading) — Intel calls it Hyperthreading — each physical core maintains two complete sets of architectural registers and two instruction frontends. Both hardware threads share the core's execution units and caches, but the CPU interleaves them to fill pipeline bubbles from one with work from the other.\n\nThe OS sees each hardware thread as a distinct logical CPU:\n• logical CPUs = sockets × physical cores per socket × hardware threads per core\n• On a 2-core hyperthreaded machine: 1 × 2 × 2 = 4 logical CPUs\n\n🔍 What logical CPUs are not\n\nTwo logical CPUs on the same physical core share execution units. If both run CPU-saturating work simultaneously, each gets roughly half the throughput of a fully uncontended core — not double. SMT helps with latency-hiding (memory stalls, I/O waits) but does not double compute capacity.\n\nPhysical cores is the ceiling for throughput on pure CPU-bound work. Logical CPUs is the ceiling for latency-hiding workloads with I/O gaps.\n\n⚙️ Why it matters for worker counts\n\navailable_parallelism() returns logical CPUs. Setting workers equal to logical CPUs saturates every hardware thread simultaneously. On SMT hardware this works well for I/O-bound workloads where threads spend time waiting, but two logical CPUs sharing a physical core do not give twice the throughput for CPU-bound tasks. The default (logical CPU count − 2) leaves two hardware threads free for the OS scheduler and the upload server's blocking thread pool.";

const FIELD_NAMES: &[&str] = &[
    "BACKEND_HOST",                   // 0
    "BACKEND_PORT",                   // 1
    "SEARCH_WORKERS",                 // 2
    "SEARCH_MAX_CONNECTIONS",         // 3
    "SEARCH_MAX_BODY_KB",             // 4
    "SEARCH_TIMEOUT_SECS",            // 5
    "TRUST_PROXY_SEARCH",             // 6
    "RATE_LIMIT_LRU_CAPACITY",        // 7
    "UPLOAD_HOST",                    // 8
    "UPLOAD_PORT",                    // 9
    "UPLOAD_WORKERS",                 // 10
    "UPLOAD_MAX_CONNECTIONS",         // 11
    "UPLOAD_MAX_CONCURRENT",          // 12
    "UPLOAD_MAX_MB",                  // 13
    "UPLOAD_TIMEOUT_SECS",            // 14
    "TRUST_PROXY_UPLOAD",             // 15
    "UPLOAD_RATE_LIMIT_LRU_CAPACITY", // 16
    "UPLOAD_CORS_ORIGINS",            // 17
    "UPLOAD_ONNX_THREADS",            // 18
];

const TIPS: &[&str] = &[
    // 0  BACKEND_HOST
    "Controls which network interface the search server's TCP socket is bound to. When the OS binds to 127.0.0.1 (loopback), it only accepts connections that originate on the same machine — no external traffic can reach it regardless of firewall rules. Binding to 0.0.0.0 tells the OS to accept connections on all available interfaces: loopback, Ethernet, WiFi, Docker bridges, VPNs, everything.\n\nUse 127.0.0.1 when ag sits behind a reverse proxy (Nginx, Caddy, Traefik) on the same machine. The proxy handles TLS and public traffic; ag only needs to be reachable from localhost. This is the most common production setup and means ag is never directly exposed even if a firewall rule is wrong.\n\nUse 0.0.0.0 when ag must be accessed directly over the network without a proxy — for example, in a bare-metal dev environment or Docker container where the port is mapped to the host. Never use 0.0.0.0 on a publicly routable interface without a firewall or rate-limiting proxy in front.",
    // 1  BACKEND_PORT
    "The TCP port number the search server listens on. Ports 0–1023 are reserved and require root privileges to bind. Ports 1024–65535 are available to user-space processes. ag defaults to 3010, chosen to be above the reserved range and unlikely to conflict with common services (Postgres 5432, Redis 6379, MySQL 3306).\n\nOnly change this if 3010 conflicts with another process on the same host. Check with: ss -tlnp | grep 3010 or lsof -i :3010. When you do change it, update BACKEND_PORT in .env, update the frontend's API base URL, and update any reverse-proxy upstream config that forwards to this port.\n\nKeeping search (3010) and upload (3011) on adjacent ports is a convention that makes firewall rules and proxy config easy to read and audit.",
    // 2  SEARCH_WORKERS
    "An Actix worker is an OS thread. When the server starts it spawns exactly this many threads, and each runs its own independent tokio async runtime. Incoming TCP connections are distributed across all workers by the OS, so each thread independently accepts, reads, parses, and responds to requests.\n\nWithin a single worker, tokio multiplexes thousands of concurrent async tasks without ever blocking the thread — the thread only burns CPU when it has real work to do (JSON parsing, vector operations, serialisation). This means one worker can sustain hundreds of simultaneous connections if those connections spend most of their time waiting on I/O.\n\nMore workers help when requests are CPU-bound: reranking, embedding lookups, high-QPS serialisation. They do not help if the bottleneck is a shared lock or a downstream roundtrip. The default (logical CPU count − 2) reserves headroom for the OS scheduler and the upload server's ONNX thread pool. Logical CPU count is what the OS scheduler sees: physical cores × hardware threads per core (SMT/hyperthreading). On a 2-core machine with hyperthreading that is 4 logical CPUs, giving a default of 2 workers — not the physical core count. Raise it if search latency climbs under load and CPU utilisation is below 100%; lower it if uploads are starved for cores.",
    // 3  SEARCH_MAX_CONNECTIONS
    "Sets the maximum number of simultaneously open TCP connections the search server will hold. When a client connects, the OS completes the TCP three-way handshake and hands the socket to Actix, which increments an internal counter. Once the limit is reached, new incoming connections are held in the OS backlog queue rather than being refused outright — from the client's perspective the connection appears to hang until an existing one closes.\n\nThis is a safety ceiling, not a performance target. Every open socket uses a small amount of kernel memory (~4 KB) and a file descriptor. Keeping this bounded prevents a connection flood from exhausting file descriptors or kernel memory.\n\nFor local development or a small team, 100–500 is more than enough. For production, set to your expected peak simultaneous connections plus about 20% headroom. If you start seeing connection timeouts under load, raise this value. If memory climbs steadily over time, suspect leaked connections and investigate before raising the limit further.",
    // 4  SEARCH_MAX_BODY_KB
    "Caps the size of the HTTP request body the search server will accept, in kilobytes. Before reading the body, Actix checks the Content-Length header against this limit. If the payload is too large, Actix returns 413 Payload Too Large immediately and closes the connection without buffering any of the body — this prevents memory exhaustion from oversized requests.\n\nTypical search queries are well under 1 KB. Even a verbose query with filters and metadata is unlikely to exceed 8 KB. The 64 KB default is deliberately generous to avoid false positives while still blocking runaway payloads.\n\nKeeping this low is a defense-in-depth measure: even if a vulnerability in a search route could be exploited via a large payload, this cap limits the damage. Never raise it to accommodate file uploads — those go to the upload server which has its own UPLOAD_MAX_MB setting. The two servers intentionally have separate size limits so a misconfiguration on one does not affect the other.",
    // 5  SEARCH_TIMEOUT_SECS
    "Controls how long Actix waits for a client to finish sending its complete HTTP request (headers plus body) before closing the connection with 408 Request Timeout. The timer starts the moment the TCP connection is accepted. If the client hasn't sent the last byte of its request body within this window, Actix drops the connection.\n\nThis is a defence against slow-client attacks (Slowloris and variants), where a malicious client opens many connections and sends data at a trickle to hold sockets open indefinitely, eventually exhausting the connection pool.\n\nNote that this timeout governs request receipt only, not how long a search operation takes to produce a response. There is no built-in response timeout — if a search handler hangs, the connection will stay open. 30 s is appropriate for almost all uses. Lower it to 5–10 s in production to fail fast and reclaim sockets from stalled clients. Only raise it if your client is on a genuinely very slow connection and legitimately takes longer than 30 s to transmit its request body (rare — most HTTP clients send the full request instantly).",
    // 6  TRUST_PROXY_SEARCH
    "Controls whether Actix trusts the X-Forwarded-For or Forwarded HTTP headers to determine the real client IP address, or uses the raw TCP peer address instead.\n\nWhen ag sits behind a reverse proxy, the proxy terminates the TCP/TLS connection and opens a new one to ag. The raw peer IP Actix sees is the proxy's IP, not the original client's. To expose the real client IP, the proxy adds an X-Forwarded-For header containing the original address. With TRUST_PROXY enabled, Actix reads that header and uses it for rate limiting and logging.\n\nThe security risk: if TRUST_PROXY is enabled and ag is not actually behind a proxy, any client can send their own X-Forwarded-For: 1.2.3.4 header and forge any IP address. They could use a different forged IP on every request, rendering per-IP rate limiting useless.\n\nDisabled is always safe — it means the TCP peer address is always used, which is correct and unforgeable. Only enable this when every path to ag goes through a proxy you control that strips and rewrites X-Forwarded-For. If in doubt, leave it off.",
    // 7  RATE_LIMIT_LRU_CAPACITY
    "Sets the number of IP address slots in the token-bucket LRU (Least Recently Used) cache that backs the search server's per-IP rate limiter.\n\nThe rate limiter gives each client IP its own token bucket — a counter that refills at a configured rate and is consumed by requests. These buckets live in an LRU cache. When a new IP arrives and the cache is full, the entry for the least recently seen IP is evicted to make room. The next request from an evicted IP gets a fresh, full bucket — effectively resetting that IP's rate-limit state.\n\nIf the cache is too small, active clients cycle in and out. A throttled IP can get evicted, reset, and bypass the limit. If the cache is large enough to hold all active IPs simultaneously, no eviction occurs during normal traffic.\n\nSet this to at least the number of distinct client IPs active in a 5-minute window. For an internal tool with a handful of users: 64–256. For a team API: 1024 (the default). For a public-facing service: 10 000+. Each slot uses roughly 100–200 bytes, so even 50 000 slots is only ~10 MB.",
    // 8  UPLOAD_HOST
    "Controls which network interface the upload server binds to. Mechanically identical to BACKEND_HOST — see that entry for a full explanation of loopback vs. all-interfaces binding.\n\nThe upload server has its own independent bind address, which enables network segmentation. A common production pattern: bind the search server to 0.0.0.0 (or a public interface) so external users can query documents, while binding the upload server to an internal-only IP (e.g. 10.0.0.1) so only trusted internal systems can ingest documents. This gives you access control at the network layer without any application-level authentication changes.\n\nFor simple setups, leave it matching BACKEND_HOST. Set it differently only when you have distinct network interfaces and want different exposure levels for the two servers.",
    // 9  UPLOAD_PORT
    "The TCP port the upload server listens on. The upload server runs as a completely separate HttpServer instance in the same process, bound to its own port. Default: 3011.\n\nChange this only if 3011 is already in use on your machine (check with ss -tlnp | grep 3011). When you do change it, update UPLOAD_PORT in .env, update the frontend's upload API base URL, and update any firewall or proxy rules that reference this port.\n\nThe port split between search (3010) and upload (3011) enables different security policies at the network layer. For example, you can open 3010 publicly in a firewall while blocking 3011, allowing external search without allowing external uploads. Keeping the two ports adjacent is a convention that makes these rules easy to read and audit.",
    // 10 UPLOAD_WORKERS
    "Same concept as SEARCH_WORKERS — each worker is an OS thread running its own tokio event loop. Workers accept upload connections, stream the request body, and hand work off to the semaphore and blocking pool.\n\nThe critical difference on the upload server: the real concurrency control is UPLOAD_MAX_CONCURRENT (the semaphore), not this setting. The semaphore is the hard gate on how many uploads are actively being processed. Workers are just the async runtime that ferries bytes in and waits for a semaphore permit. Having more workers than UPLOAD_MAX_CONCURRENT means extra threads spend nearly all their time blocked on the semaphore, adding OS overhead without adding throughput.\n\nKeep this at or just below UPLOAD_MAX_CONCURRENT. The default of 2 is correct for most setups: each worker can hold multiple permits simultaneously because the actual heavy work — ONNX embedding — is offloaded to the blocking thread pool, not the worker thread itself. Only raise to 4 if you have fast NVMe storage, 8+ logical CPUs, and have already raised UPLOAD_MAX_CONCURRENT.",
    // 11 UPLOAD_MAX_CONNECTIONS
    "Maximum simultaneous open TCP connections to the upload server. Mechanically the same as SEARCH_MAX_CONNECTIONS — connections beyond this limit queue in the OS backlog rather than being refused.\n\nThis limit is intentionally low by default because upload connections are expensive. Each active upload holds a file buffer in memory and may be competing for an ONNX semaphore slot. A low cap here prevents a flood of upload connections from exhausting memory before the semaphore can do its job.\n\nImportant: raising this does not increase throughput. The real throughput limit is UPLOAD_MAX_CONCURRENT. Raising MAX_CONNECTIONS without raising MAX_CONCURRENT just allows more connections to queue while waiting for a semaphore permit — it burns memory without helping latency. Only raise MAX_CONNECTIONS if you are seeing TCP-level connection failures (as opposed to 503 responses, which come from the semaphore). 50 is appropriate for most deployments.",
    // 12 UPLOAD_MAX_CONCURRENT
    "The number of permits in the semaphore that gates active upload processing. This is the single most important upload performance and stability knob.\n\nWhen an upload request passes the rate limiter, it tries to acquire a permit from this semaphore. If no permits are free, Actix returns 503 Service Unavailable immediately with a Retry-After: 1 header rather than queuing the request — this is intentional backpressure. A permit held means the upload is actively being processed: the file is being parsed, chunked, embedded via ONNX, and written to Tantivy. The permit is released only when all of that work is complete.\n\nWhy 503 instead of queuing? ONNX inference and index writes are both memory-intensive. If N uploads were queued and processed serially, all N files would be buffered in RAM simultaneously. Returning 503 immediately tells the client to retry and keeps memory usage predictable. The client retries after 1 s and usually gets a permit on the next attempt.\n\nSet to min(available CPU cores for upload, UPLOAD_ONNX_THREADS). Typical sweet spot is 2–6. Raising beyond UPLOAD_ONNX_THREADS does nothing — the ONNX blocking pool becomes the bottleneck and uploads queue there instead. Raising beyond available CPU cores causes CPU thrashing from simultaneous embedding runs.",
    // 13 UPLOAD_MAX_MB
    "Maximum HTTP request body size for the upload server, in megabytes. Actix checks Content-Length against this limit before reading the body and returns 413 Payload Too Large immediately if exceeded.\n\nFor multipart/form-data file uploads (how ag sends documents), the entire file plus form overhead must fit within this limit. The overhead is small — typically a few hundred bytes — so the limit is effectively the maximum file size.\n\nSet to about 1.5× your largest expected document. The safety margin covers multipart encoding overhead and avoids edge cases. If your largest PDF is 80 MB, use 120 MB. If uploads fail with 413, the file exceeds this limit — raise it. If the server is killed with OOM during uploads, this limit may be too high relative to available RAM, especially if INDEX_IN_RAM is enabled (which keeps the full index in heap memory alongside the upload buffer). In that case, either lower this limit, disable INDEX_IN_RAM, or add more RAM.",
    // 14 UPLOAD_TIMEOUT_SECS
    "How long Actix waits for a client to finish transmitting the complete upload request (headers plus the entire file body) before returning 408 Request Timeout. The timer starts at TCP connection and runs until the last byte of the request body is received. After reception is complete and processing begins, there is no separate processing timeout — the connection stays open until the response is sent.\n\nThis protects against stalled or very slow upload clients holding connections open indefinitely. Unlike the search timeout, this must be long enough to accommodate large file transfers over slow networks.\n\nHow to set it: estimate (max file size MB ÷ slowest realistic upload speed MB/s) × 2. Examples: a 150 MB file at 10 MB/s LAN takes 15 s — use 30 s. The same file at 1 MB/s takes 150 s — use 300 s (the default). At 0.5 MB/s it takes 300 s — use 600 s. Base the calculation on your slowest expected client, not your average client. A timeout that is too short causes mysterious upload failures on slow connections that are indistinguishable from network errors on the client side.",
    // 15 TRUST_PROXY_UPLOAD
    "Controls whether the upload server trusts X-Forwarded-For / Forwarded headers for client IP determination. Mechanically identical to TRUST_PROXY_SEARCH — see that entry for a full explanation of the security model.\n\nThe upload server has its own independent trust setting for two reasons. First, you might deploy the two servers behind different proxies — for example, the search server behind a CDN and the upload server behind an internal load balancer that only accepts connections from the corporate network. Different trust levels may be correct for each. Second, mistakes in one setting do not affect the other server.\n\nThe same fundamental rule applies: disabled is always safe. Enabled is only correct when every path to this server goes through a proxy you control that strips client-supplied X-Forwarded-For headers and replaces them with the verified client IP. If the upload server is on an internal-only interface with no proxy in front, leave this disabled — the raw peer IP is already correct and unforgeable.",
    // 16 UPLOAD_RATE_LIMIT_LRU_CAPACITY
    "The number of IP address slots in the token-bucket LRU cache for the upload server's rate limiter. Independent from the search server's LRU so upload and search traffic don't interfere with each other's rate-limit state.\n\nMechanically identical to RATE_LIMIT_LRU_CAPACITY for search — see that entry for a full explanation of how the LRU eviction works and why undersizing it allows throttled IPs to reset their buckets.\n\nThe default of 256 is lower than the search default of 1024 because upload traffic volume is typically much lower. A small number of systems or users ingest documents; many more query them. 256 slots simultaneously tracks 256 distinct upload client IPs.\n\nSet to the number of distinct IPs you expect to be actively uploading within any 5-minute window. For a single-user or small internal tool: 32–64. For a team: 256 (default). For a multi-tenant service where many different users upload documents: 1 000+. Each slot uses ~100–200 bytes, so even 10 000 slots is about 2 MB.",
    // 17 UPLOAD_CORS_ORIGINS
    "A comma-separated list of origins the browser is allowed to make cross-origin upload requests from. Controls the Access-Control-Allow-Origin response header on the upload server.\n\nCORS (Cross-Origin Resource Sharing) is a browser security mechanism. When JavaScript on origin A (e.g. https://app.example.com) makes an HTTP request to server B (e.g. http://localhost:3011), the browser first sends an OPTIONS preflight. The server responds with which origins are permitted. If the requesting origin is not on the list, the browser blocks the response — even if the server returned 200. This only affects browser clients; curl and server-to-server calls are not subject to CORS.\n\nLeave empty to allow any origin (Access-Control-Allow-Origin: *). This is appropriate for local development or when the upload server is on an internal network not reachable from the public internet.\n\nIn production, set this to your frontend's exact origin, e.g. https://app.example.com. This prevents a malicious third-party website from using a logged-in user's browser to silently upload files to your server. Precision matters: the scheme, hostname, and port must all match exactly. https://example.com and https://www.example.com are different origins. Multiple origins: comma-separated with no extra spaces.",
    // 18 UPLOAD_ONNX_THREADS
    "Controls the size of the dedicated blocking thread pool used to run ONNX embedding inference during document uploads.\n\nRust's async runtimes (tokio) are designed for non-blocking I/O. If you run ONNX inference directly on a tokio worker thread, it blocks that thread for the entire duration of the computation — typically 50–500 ms per chunk — and starves all other async tasks assigned to that worker. To prevent this, ag uses tokio::task::spawn_blocking to dispatch inference to a separate pool of threads that are allowed to block. UPLOAD_ONNX_THREADS is the size of that pool.\n\nThe coupling with UPLOAD_MAX_CONCURRENT is tight: each upload that holds a semaphore permit needs one blocking thread to run its inference. If ONNX_THREADS < MAX_CONCURRENT, some permits are held but no blocking thread is free — those uploads stall inside spawn_blocking waiting for a thread slot. If ONNX_THREADS > MAX_CONCURRENT, threads sit idle permanently because the semaphore will never admit more uploads than permits available. The two values should always be equal.\n\nMemory note: each concurrent ONNX inference loads a batch of text through the embedding model. Depending on the model, this is roughly 100–400 MB of RAM per concurrent run. Before raising both UPLOAD_ONNX_THREADS and UPLOAD_MAX_CONCURRENT above 4, verify you have enough free RAM to sustain that many simultaneous inference passes.",
];

#[component]
pub fn ConfigOther() -> Element {
    // Search fields
    let mut s_host       = use_signal(|| String::new());
    let mut s_port       = use_signal(|| String::new());
    let mut s_workers    = use_signal(|| String::new());
    let mut s_max_conn   = use_signal(|| String::new());
    let mut s_body_kb    = use_signal(|| String::new());
    let mut s_timeout    = use_signal(|| String::new());
    let mut s_proxy      = use_signal(|| false);
    let mut s_lru        = use_signal(|| String::new());

    // Upload fields
    let mut u_host        = use_signal(|| String::new());
    let mut u_port        = use_signal(|| String::new());
    let mut u_workers     = use_signal(|| String::new());
    let mut u_max_conn    = use_signal(|| String::new());
    let mut u_max_conc    = use_signal(|| String::new());
    let mut u_max_mb      = use_signal(|| String::new());
    let mut u_timeout     = use_signal(|| String::new());
    let mut u_proxy       = use_signal(|| false);
    let mut u_lru         = use_signal(|| String::new());
    let mut u_cors        = use_signal(|| String::new());
    let mut u_onnx        = use_signal(|| String::new());

    let mut loaded               = use_signal(|| false);
    let mut saving               = use_signal(|| false);
    let mut save_msg: Signal<Option<(bool, String)>> = use_signal(|| None);
    let mut show_info            = use_signal(|| false);
    let mut show_restart_confirm = use_signal(|| false);
    let mut restart_msg: Signal<Option<String>> = use_signal(|| None);
    // Drives the single shared per-field modal; None = closed.
    let mut active_info: Signal<Option<u8>> = use_signal(|| None);
    // OS thread reference modal — opened from within the worker field modals.
    let mut show_os_thread_ref: Signal<bool> = use_signal(|| false);
    // Downstream roundtrip reference modal — opened from within SEARCH_WORKERS modal.
    let mut show_downstream_ref: Signal<bool> = use_signal(|| false);
    // Logical CPU reference modal — opened from within worker field modals.
    let mut show_logical_cpu_ref: Signal<bool> = use_signal(|| false);
    // Semaphore etymology modal — opened from within the UPLOAD_WORKERS modal.
    let mut show_semaphore_ref: Signal<bool> = use_signal(|| false);
    // .env.server explanation modal — opened from the save button row.
    let mut show_env_server_info: Signal<bool> = use_signal(|| false);

    use_future(move || async move {
        if let Ok(cfg) = api::fetch_server_config().await {
            s_host.set(cfg.search.host);
            s_port.set(cfg.search.port.to_string());
            s_workers.set(cfg.search.workers.to_string());
            s_max_conn.set(cfg.search.max_connections.to_string());
            s_body_kb.set(cfg.search.max_body_kb.to_string());
            s_timeout.set(cfg.search.timeout_secs.to_string());
            s_proxy.set(cfg.search.trust_proxy);
            s_lru.set(cfg.search.rate_limit_lru_capacity.to_string());
            u_host.set(cfg.upload.host);
            u_port.set(cfg.upload.port.to_string());
            u_workers.set(cfg.upload.workers.to_string());
            u_max_conn.set(cfg.upload.max_connections.to_string());
            u_max_conc.set(cfg.upload.max_concurrent.to_string());
            u_max_mb.set(cfg.upload.max_mb.to_string());
            u_timeout.set(cfg.upload.timeout_secs.to_string());
            u_proxy.set(cfg.upload.trust_proxy);
            u_lru.set(cfg.upload.rate_limit_lru_capacity.to_string());
            u_cors.set(cfg.upload.cors_origins.join(", "));
            u_onnx.set(cfg.upload.onnx_threads.to_string());
            loaded.set(true);
        }
    });

    let on_save = move |_| {
        spawn(async move {
            saving.set(true);
            save_msg.set(None);
            let mut p = std::collections::HashMap::new();
            p.insert("BACKEND_HOST".into(),                   s_host());
            p.insert("BACKEND_PORT".into(),                   s_port());
            p.insert("SEARCH_WORKERS".into(),                 s_workers());
            p.insert("SEARCH_MAX_CONNECTIONS".into(),         s_max_conn());
            p.insert("SEARCH_MAX_BODY_KB".into(),             s_body_kb());
            p.insert("SEARCH_TIMEOUT_SECS".into(),            s_timeout());
            p.insert("TRUST_PROXY_SEARCH".into(),             s_proxy().to_string());
            p.insert("RATE_LIMIT_LRU_CAPACITY".into(),        s_lru());
            p.insert("UPLOAD_HOST".into(),                    u_host());
            p.insert("UPLOAD_PORT".into(),                    u_port());
            p.insert("UPLOAD_WORKERS".into(),                 u_workers());
            p.insert("UPLOAD_MAX_CONNECTIONS".into(),         u_max_conn());
            p.insert("UPLOAD_MAX_CONCURRENT".into(),          u_max_conc());
            p.insert("UPLOAD_MAX_MB".into(),                  u_max_mb());
            p.insert("UPLOAD_TIMEOUT_SECS".into(),            u_timeout());
            p.insert("TRUST_PROXY_UPLOAD".into(),             u_proxy().to_string());
            p.insert("UPLOAD_RATE_LIMIT_LRU_CAPACITY".into(), u_lru());
            p.insert("UPLOAD_CORS_ORIGINS".into(),            u_cors());
            p.insert("UPLOAD_ONNX_THREADS".into(),            u_onnx());
            match api::save_server_config(p).await {
                Ok(_)  => save_msg.set(Some((true,  "Saved — restart backend to apply".into()))),
                Err(e) => save_msg.set(Some((false, format!("Error: {e}")))),
            }
            saving.set(false);
        });
    };

    let open = move |n: u8| move |_: Event<MouseData>| active_info.set(Some(n));
    let close_field_modal = move |_: Event<MouseData>| {
        active_info.set(None);
        show_os_thread_ref.set(false);
        show_downstream_ref.set(false);
        show_logical_cpu_ref.set(false);
        show_semaphore_ref.set(false);
    };

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                    BreadcrumbItem::new("Actix", Some(Route::ConfigOther {})),
                ],
            }

            ConfigNav { active: ConfigTab::Other }

            Panel { title: None, refresh: None,
                div { class: "flex items-center gap-2 mb-4",
                    h3 { class: "text-sm font-semibold text-gray-200", "Actix Tuning" }
                    button {
                        class: PARAM_ICON_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: move |_| show_info.set(true),
                        InfoIcon {}
                    }
                }

                if !loaded() {
                    div { class: "text-gray-400 text-sm", "Loading…" }
                } else {
                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-8",

                        // ── Search :3010 ───────────────────────────────────
                        div {
                            class: "grid gap-x-2 gap-y-2 items-center",
                            style: "grid-template-columns: max-content min-content auto;",

                            div { class: "flex items-center gap-2 col-span-3 pb-1",
                                span { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide", "Search" }
                                span { class: "text-xs font-mono text-teal-400", ":{s_port()}" }
                            }

                            label { class: LABEL, "BACKEND_HOST" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(0), InfoIcon {} }
                            input { class: INPUT_MD, value: "{s_host()}", oninput: move |e| s_host.set(e.value()) }

                            label { class: LABEL, "BACKEND_PORT" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(1), InfoIcon {} }
                            input { r#type: "number", class: INPUT_SM, value: "{s_port()}", oninput: move |e| s_port.set(e.value()) }

                            label { class: LABEL, "SEARCH_WORKERS" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(2), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{s_workers()}", oninput: move |e| s_workers.set(e.value()) }

                            label { class: LABEL, "SEARCH_MAX_CONNECTIONS" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(3), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{s_max_conn()}", oninput: move |e| s_max_conn.set(e.value()) }

                            label { class: LABEL, "SEARCH_MAX_BODY_KB" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(4), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{s_body_kb()}", oninput: move |e| s_body_kb.set(e.value()) }

                            label { class: LABEL, "SEARCH_TIMEOUT_SECS" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(5), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{s_timeout()}", oninput: move |e| s_timeout.set(e.value()) }

                            label { class: LABEL, "TRUST_PROXY_SEARCH" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(6), InfoIcon {} }
                            input { r#type: "checkbox", class: "checkbox checkbox-xs border-2 border-gray-200", checked: s_proxy(), onchange: move |e| s_proxy.set(e.checked()) }

                            label { class: LABEL, "RATE_LIMIT_LRU_CAPACITY" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(7), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{s_lru()}", oninput: move |e| s_lru.set(e.value()) }

                            div { class: "col-span-3 pt-1" }
                            button {
                                class: "btn btn-sm",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                                disabled: saving(),
                                onclick: on_save,
                                if saving() { "Saving…" } else { "Save to .env.server" }
                            }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_env_server_info.set(true),
                                InfoIcon {}
                            }
                            div { class: "flex items-center gap-3 flex-wrap",
                                button {
                                    class: "btn btn-sm btn-ghost text-gray-300 border border-gray-600",
                                    onclick: move |_| show_restart_confirm.set(true),
                                    "Restart to apply"
                                }
                                if let Some((ok, msg)) = save_msg() {
                                    span {
                                        class: if ok { "text-xs text-green-400" } else { "text-xs text-red-400" },
                                        "{msg}"
                                    }
                                }
                                if let Some(msg) = restart_msg() {
                                    span { class: "text-xs text-yellow-400", "{msg}" }
                                }
                            }
                        }

                        // ── Upload :3011 ───────────────────────────────────
                        div {
                            class: "grid gap-x-2 gap-y-2 items-center",
                            style: "grid-template-columns: max-content min-content auto;",

                            div { class: "flex items-center gap-2 col-span-3 pb-1",
                                span { class: "text-xs font-semibold text-gray-300 uppercase tracking-wide", "Upload" }
                                span { class: "text-xs font-mono text-teal-400", ":{u_port()}" }
                            }

                            label { class: LABEL, "UPLOAD_HOST" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(8), InfoIcon {} }
                            input { class: INPUT_MD, value: "{u_host()}", oninput: move |e| u_host.set(e.value()) }

                            label { class: LABEL, "UPLOAD_PORT" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(9), InfoIcon {} }
                            input { r#type: "number", class: INPUT_SM, value: "{u_port()}", oninput: move |e| u_port.set(e.value()) }

                            label { class: LABEL, "UPLOAD_WORKERS" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(10), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{u_workers()}", oninput: move |e| u_workers.set(e.value()) }

                            label { class: LABEL, "UPLOAD_MAX_CONNECTIONS" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(11), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{u_max_conn()}", oninput: move |e| u_max_conn.set(e.value()) }

                            label { class: LABEL, "UPLOAD_MAX_CONCURRENT" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(12), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{u_max_conc()}", oninput: move |e| u_max_conc.set(e.value()) }

                            label { class: LABEL, "UPLOAD_MAX_MB" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(13), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{u_max_mb()}", oninput: move |e| u_max_mb.set(e.value()) }

                            label { class: LABEL, "UPLOAD_TIMEOUT_SECS" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(14), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{u_timeout()}", oninput: move |e| u_timeout.set(e.value()) }

                            label { class: LABEL, "TRUST_PROXY_UPLOAD" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(15), InfoIcon {} }
                            input { r#type: "checkbox", class: "checkbox checkbox-xs border-2 border-gray-200", checked: u_proxy(), onchange: move |e| u_proxy.set(e.checked()) }

                            label { class: LABEL, "UPLOAD_RATE_LIMIT_LRU_CAPACITY" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(16), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{u_lru()}", oninput: move |e| u_lru.set(e.value()) }

                            label { class: LABEL, "UPLOAD_CORS_ORIGINS" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(17), InfoIcon {} }
                            input { class: INPUT_LG, placeholder: "leave empty for any origin", value: "{u_cors()}", oninput: move |e| u_cors.set(e.value()) }

                            label { class: LABEL, "UPLOAD_ONNX_THREADS" }
                            button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: open(18), InfoIcon {} }
                            input { r#type: "number", min: "1", class: INPUT_SM, value: "{u_onnx()}", oninput: move |e| u_onnx.set(e.value()) }
                        }
                    }

                }
            }

            // ── Per-field info modal ──────────────────────────────────────
            if let Some(idx) = active_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: close_field_modal,
                    div {
                        class: "bg-gray-900 border border-gray-700 rounded-lg p-5 w-96 shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        // title row
                        div { class: "flex items-center justify-between mb-3",
                            h3 { class: "text-sm font-semibold text-gray-100 font-mono",
                                "{FIELD_NAMES[idx as usize]}"
                            }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-lg leading-none",
                                onclick: close_field_modal,
                                "✕"
                            }
                        }
                        div { class: "space-y-2 mb-4",
                            if idx == 2 || idx == 10 {
                                {
                                    let tip = TIPS[idx as usize];
                                    let first = tip.split("\n\n").next().unwrap_or("");
                                    if let Some((before, after)) = first.split_once("OS thread") {
                                        if idx == 10 {
                                            if let Some((mid, after_sem)) = after.split_once("semaphore") {
                                                rsx! {
                                                    p { class: "text-sm text-gray-300 leading-relaxed",
                                                        "{before}"
                                                        a {
                                                            class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-medium",
                                                            onclick: move |e: Event<MouseData>| {
                                                                e.stop_propagation();
                                                                show_os_thread_ref.set(true);
                                                            },
                                                            "OS thread"
                                                        }
                                                        "{mid}"
                                                        a {
                                                            class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-medium",
                                                            onclick: move |e: Event<MouseData>| {
                                                                e.stop_propagation();
                                                                show_semaphore_ref.set(true);
                                                            },
                                                            "semaphore"
                                                        }
                                                        "{after_sem}"
                                                    }
                                                }
                                            } else {
                                                rsx! {
                                                    p { class: "text-sm text-gray-300 leading-relaxed",
                                                        "{before}"
                                                        a {
                                                            class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-medium",
                                                            onclick: move |e: Event<MouseData>| {
                                                                e.stop_propagation();
                                                                show_os_thread_ref.set(true);
                                                            },
                                                            "OS thread"
                                                        }
                                                        "{after}"
                                                    }
                                                }
                                            }
                                        } else {
                                            rsx! {
                                                p { class: "text-sm text-gray-300 leading-relaxed",
                                                    "{before}"
                                                    a {
                                                        class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-medium",
                                                        onclick: move |e: Event<MouseData>| {
                                                            e.stop_propagation();
                                                            show_os_thread_ref.set(true);
                                                        },
                                                        "OS thread"
                                                    }
                                                    "{after}"
                                                }
                                            }
                                        }
                                    } else {
                                        rsx! { p { class: "text-sm text-gray-300 leading-relaxed", "{first}" } }
                                    }
                                }
                                for (para_i, para) in TIPS[idx as usize].split("\n\n").skip(1).enumerate() {
                                    if idx == 2 && para_i == 1 {
                                        {
                                            if let Some((b1, a1)) = para.split_once("downstream roundtrip") {
                                                if let Some((mid, a2)) = a1.split_once("logical CPU count") {
                                                    rsx! {
                                                        p { class: "text-sm text-gray-300 leading-relaxed",
                                                            "{b1}"
                                                            a {
                                                                class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-medium",
                                                                onclick: move |e: Event<MouseData>| {
                                                                    e.stop_propagation();
                                                                    show_downstream_ref.set(true);
                                                                },
                                                                "downstream roundtrip"
                                                            }
                                                            "{mid}"
                                                            a {
                                                                class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-medium",
                                                                onclick: move |e: Event<MouseData>| {
                                                                    e.stop_propagation();
                                                                    show_logical_cpu_ref.set(true);
                                                                },
                                                                "logical CPU count"
                                                            }
                                                            "{a2}"
                                                        }
                                                    }
                                                } else {
                                                    rsx! {
                                                        p { class: "text-sm text-gray-300 leading-relaxed",
                                                            "{b1}"
                                                            a {
                                                                class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-medium",
                                                                onclick: move |e: Event<MouseData>| {
                                                                    e.stop_propagation();
                                                                    show_downstream_ref.set(true);
                                                                },
                                                                "downstream roundtrip"
                                                            }
                                                            "{a1}"
                                                        }
                                                    }
                                                }
                                            } else {
                                                rsx! { p { class: "text-sm text-gray-300 leading-relaxed", "{para}" } }
                                            }
                                        }
                                    } else if idx == 10 && para_i == 1 {
                                        {
                                            if let Some((before, after)) = para.split_once("logical CPUs") {
                                                rsx! {
                                                    p { class: "text-sm text-gray-300 leading-relaxed",
                                                        "{before}"
                                                        a {
                                                            class: "text-blue-400 hover:text-blue-300 underline cursor-pointer font-medium",
                                                            onclick: move |e: Event<MouseData>| {
                                                                e.stop_propagation();
                                                                show_logical_cpu_ref.set(true);
                                                            },
                                                            "logical CPUs"
                                                        }
                                                        "{after}"
                                                    }
                                                }
                                            } else {
                                                rsx! { p { class: "text-sm text-gray-300 leading-relaxed", "{para}" } }
                                            }
                                        }
                                    } else {
                                        p { class: "text-sm text-gray-300 leading-relaxed", "{para}" }
                                    }
                                }
                            } else {
                                for para in TIPS[idx as usize].split("\n\n") {
                                    p { class: "text-sm text-gray-300 leading-relaxed", "{para}" }
                                }
                            }
                        }
                        button {
                            class: "btn btn-sm w-full",
                            style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                            onclick: close_field_modal,
                            "Got it"
                        }
                    }
                }
            }

            // ── OS thread reference modal (opens from worker field modals) ─
            if show_os_thread_ref() {
                div {
                    class: "fixed inset-0 z-[60] flex items-center justify-center bg-black/70",
                    onclick: move |_| show_os_thread_ref.set(false),
                    div {
                        class: "bg-gray-900 border border-gray-700 rounded-lg p-5 w-[36rem] shadow-2xl max-h-[80vh] flex flex-col",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-3 flex-shrink-0",
                            h3 { class: "text-sm font-semibold text-gray-100", "OS Thread — concept reference" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-lg leading-none",
                                onclick: move |_| show_os_thread_ref.set(false),
                                "✕"
                            }
                        }
                        div { class: "overflow-y-auto flex-1 space-y-1 pr-1",
                            for (label, desc) in OS_THREAD_LINKS {
                                div { class: "flex gap-2 items-baseline py-0.5",
                                    span { class: "text-xs text-gray-400 font-mono whitespace-nowrap flex-shrink-0", "{label}" }
                                    span { class: "text-xs text-gray-300 leading-relaxed", "{desc}" }
                                }
                            }
                        }
                        button {
                            class: "btn btn-sm w-full mt-4 flex-shrink-0",
                            style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                            onclick: move |_| show_os_thread_ref.set(false),
                            "Got it"
                        }
                    }
                }
            }

            // ── Downstream roundtrip reference modal ──────────────────────
            if show_downstream_ref() {
                div {
                    class: "fixed inset-0 z-[60] flex items-center justify-center bg-black/70",
                    onclick: move |_| show_downstream_ref.set(false),
                    div {
                        class: "bg-gray-900 border border-gray-700 rounded-lg p-5 w-[40rem] shadow-2xl max-h-[80vh] flex flex-col",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-3 flex-shrink-0",
                            h3 { class: "text-sm font-semibold text-gray-100", "Downstream roundtrip" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-lg leading-none",
                                onclick: move |_| show_downstream_ref.set(false),
                                "✕"
                            }
                        }
                        div { class: "overflow-y-auto flex-1 space-y-2 pr-1",
                            for para in DOWNSTREAM_ROUNDTRIP_CONTENT.split("\n\n") {
                                p { class: "text-sm text-gray-300 leading-relaxed whitespace-pre-line", "{para}" }
                            }
                        }
                        button {
                            class: "btn btn-sm w-full mt-4 flex-shrink-0",
                            style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                            onclick: move |_| show_downstream_ref.set(false),
                            "Got it"
                        }
                    }
                }
            }

            // ── Logical CPU reference modal ───────────────────────────────
            if show_logical_cpu_ref() {
                div {
                    class: "fixed inset-0 z-[60] flex items-center justify-center bg-black/70",
                    onclick: move |_| show_logical_cpu_ref.set(false),
                    div {
                        class: "bg-gray-900 border border-gray-700 rounded-lg p-5 w-[42rem] shadow-2xl max-h-[80vh] flex flex-col",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-3 flex-shrink-0",
                            h3 { class: "text-sm font-semibold text-gray-100", "Logical CPU" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-lg leading-none",
                                onclick: move |_| show_logical_cpu_ref.set(false),
                                "✕"
                            }
                        }
                        div { class: "overflow-y-auto flex-1 space-y-2 pr-1",
                            for para in LOGICAL_CPU_CONTENT.split("\n\n") {
                                p { class: "text-sm text-gray-300 leading-relaxed whitespace-pre-line", "{para}" }
                            }
                        }
                        button {
                            class: "btn btn-sm w-full mt-4 flex-shrink-0",
                            style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                            onclick: move |_| show_logical_cpu_ref.set(false),
                            "Got it"
                        }
                    }
                }
            }

            // ── Semaphore etymology modal ─────────────────────────────────
            if show_semaphore_ref() {
                div {
                    class: "fixed inset-0 z-[60] flex items-center justify-center bg-black/70",
                    onclick: move |_| show_semaphore_ref.set(false),
                    div {
                        class: "bg-gray-900 border border-gray-700 rounded-lg p-5 w-[44rem] shadow-2xl max-h-[80vh] flex flex-col",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-4 flex-shrink-0",
                            h3 { class: "text-sm font-semibold text-gray-100", "Semaphore — etymology" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-lg leading-none",
                                onclick: move |_| show_semaphore_ref.set(false),
                                "\u{2715}"
                            }
                        }
                        div { class: "overflow-y-auto flex-1 pr-1",
                            table { class: "w-full text-sm border-collapse",
                                thead {
                                    tr {
                                        th { class: "text-left text-xs text-gray-400 font-mono pb-2 pr-6 whitespace-nowrap", "Term" }
                                        th { class: "text-left text-xs text-gray-400 font-mono pb-2", "Definition" }
                                    }
                                }
                                tbody {
                                    for (term, def) in SEMAPHORE_ETYMOLOGY {
                                        tr { class: "border-t border-gray-800",
                                            td { class: "py-2 pr-6 text-xs font-mono text-blue-300 whitespace-nowrap align-top", "{term}" }
                                            td { class: "py-2 text-xs text-gray-300 leading-relaxed", "{def}" }
                                        }
                                    }
                                }
                            }
                        }
                        button {
                            class: "btn btn-sm w-full mt-4 flex-shrink-0",
                            style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                            onclick: move |_| show_semaphore_ref.set(false),
                            "Got it"
                        }
                    }
                }
            }

            // ── .env.server info modal ───────────────────────────────────
            if show_env_server_info() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_env_server_info.set(false),
                    div {
                        class: "bg-gray-900 border border-gray-700 rounded-lg p-5 w-[38rem] shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-3",
                            h3 { class: "text-sm font-semibold text-gray-100 font-mono", ".env.server" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-lg leading-none",
                                onclick: move |_| show_env_server_info.set(false),
                                "\u{2715}"
                            }
                        }
                        div { class: "space-y-2 mb-4",
                            p { class: "text-sm text-gray-300 leading-relaxed",
                                "ag reads its configuration from two files when it starts: "
                                span { class: "font-mono text-gray-100", ".env" }
                                " first, then "
                                span { class: "font-mono text-gray-100", ".env.server" }
                                " on top of it. Values in "
                                span { class: "font-mono text-gray-100", ".env.server" }
                                " override anything already set in "
                                span { class: "font-mono text-gray-100", ".env" }
                                "."
                            }
                            p { class: "text-sm text-gray-300 leading-relaxed",
                                "This split keeps concerns separate. "
                                span { class: "font-mono text-gray-100", ".env" }
                                " holds stable settings — API keys, database addresses, feature flags — and is typically version-controlled or managed by a secrets system. "
                                span { class: "font-mono text-gray-100", ".env.server" }
                                " holds machine-specific performance tuning (worker counts, connection limits, timeouts) that varies per host and can be edited without touching sensitive credentials."
                            }
                            p { class: "text-sm text-gray-300 leading-relaxed",
                                "Saving here writes only the fields on this page to "
                                span { class: "font-mono text-gray-100", ".env.server" }
                                ". The file is created if it does not exist. All other config files are left untouched. Changes take effect on the next app restart — the running app is not affected until then."
                            }
                        }
                        button {
                            class: "btn btn-sm w-full",
                            style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                            onclick: move |_| show_env_server_info.set(false),
                            "Got it"
                        }
                    }
                }
            }

            // ── Restart confirm modal ─────────────────────────────────────
            if show_restart_confirm() {
                div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_restart_confirm.set(false),
                    div {
                        class: "bg-gray-900 border border-gray-700 rounded-lg p-6 w-80 shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        h2 { class: "text-base font-bold text-gray-100 mb-2", "Restart app?" }
                        p { class: "text-sm text-gray-300 mb-4",
                            "The app will restart to apply server config changes. Active requests will be dropped."
                        }
                        div { class: "flex gap-2",
                            button {
                                class: "btn btn-sm flex-1",
                                style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                                onclick: move |_| {
                                    show_restart_confirm.set(false);
                                    spawn(async move {
                                        match api::restart_service().await {
                                            Ok(()) => restart_msg.set(Some("Restarting…".into())),
                                            Err(e) => restart_msg.set(Some(format!("Error: {}", e))),
                                        }
                                    });
                                },
                                "Yes, restart"
                            }
                            button {
                                class: "btn btn-sm flex-1 btn-ghost text-gray-300",
                                onclick: move |_| show_restart_confirm.set(false),
                                "Cancel"
                            }
                        }
                    }
                }
            }

            // ── Panel-level info modal ────────────────────────────────────
            if show_info() {
                {
                    info_modal(
                        "Actix Tuning",
                        show_info,
                        vec![
                            "Actix Web is the async Rust HTTP server framework powering both ag servers. Each HttpServer spawns a pool of OS threads (workers), and each worker multiplexes thousands of async connections using tokio.",
                            "## Search server (port 3010)",
                            "Handles all query, retrieval, monitoring, and config routes. High read concurrency — give it most of the CPU workers. Requests are cheap and fast.",
                            "• SEARCH_WORKERS — Actix worker threads. Default: CPU count − 2 (min 1).\n• SEARCH_MAX_CONNECTIONS — Max simultaneous TCP connections before new ones are queued. Default: 1000.\n• SEARCH_MAX_BODY_KB — Request body size cap (prevents large payloads on query routes). Default: 64 KB.\n• SEARCH_TIMEOUT_SECS — How long Actix waits for a client to send its full request. Default: 30 s.\n• RATE_LIMIT_LRU_CAPACITY — Slots in the per-IP token-bucket LRU. Default: 1024.",
                            "## Upload server (port 3011)",
                            "Handles document ingestion, reindex, and delete. Requests are expensive (ONNX embedding, disk I/O). Keep workers low; use UPLOAD_MAX_CONCURRENT as the real concurrency knob.",
                            "• UPLOAD_WORKERS — Actix worker threads. Default: 2.\n• UPLOAD_MAX_CONNECTIONS — Max simultaneous TCP connections. Default: 50.\n• UPLOAD_MAX_CONCURRENT — Semaphore across all workers — the actual limit on in-flight upload processing. Default: 4.\n• UPLOAD_MAX_MB — Maximum upload body size in megabytes. Default: 150 MB.\n• UPLOAD_TIMEOUT_SECS — Request receive timeout. Long for large file uploads. Default: 300 s.\n• UPLOAD_RATE_LIMIT_LRU_CAPACITY — Separate LRU from the search limiter. Default: 256.\n• UPLOAD_ONNX_THREADS — Blocking thread-pool size for ONNX embedding. Should not exceed UPLOAD_MAX_CONCURRENT. Default: 4.",
                            "## Proxy trust",
                            "TRUST_PROXY_SEARCH / TRUST_PROXY_UPLOAD control whether Actix reads the real client IP from X-Forwarded-For / Forwarded headers. Enable only when the server sits behind a trusted reverse proxy — if set incorrectly, clients can spoof their IP and bypass per-IP rate limiting.",
                            "## UPLOAD_CORS_ORIGINS",
                            "Comma-separated list of allowed origins for the upload server. Leave empty to allow any origin (default). Set to e.g. https://yourdomain.com to restrict browser-initiated uploads to known frontends.",
                            "Changes are written to .env.server and take effect on the next backend restart.",
                        ],
                    )
                }
            }
        }
    }
}
