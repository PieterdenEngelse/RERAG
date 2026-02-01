//! Documentation - Index page

use dioxus::prelude::*;

#[component]
pub fn DocuIndex() -> Element {
    let mut show_embeddings_modal = use_signal(|| false);
    let mut show_knowledge_graphs_modal = use_signal(|| false);
    let mut show_onnx_modal = use_signal(|| false);
    let mut show_onnx_params_modal = use_signal(|| false);
    let mut show_io_uring_modal = use_signal(|| false);
    let mut show_bias_modal = use_signal(|| false);
    let mut show_threads_modal = use_signal(|| false);
    let mut show_entities_production_modal = use_signal(|| false);

    rsx! {
        div { class: "min-h-screen bg-base-200 p-6",

            div { class: "max-w-4xl mx-auto",

                a {
                    href: "/docu",
                    class: "text-primary hover:underline mb-4 inline-block",
                    "← Back to Documentation"
                }

                h1 { class: "text-3xl font-bold mb-6", "📇 Index" }

                div { class: "space-y-2",

                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_embeddings_modal.set(true),
                        "Embeddings"
                    }

                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_knowledge_graphs_modal.set(true),
                        "Knowledge Graphs"
                    }

                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_onnx_modal.set(true),
                        "ONNX"
                    }

                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_onnx_params_modal.set(true),
                        "ONNX parameters"
                    }

                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_io_uring_modal.set(true),
                        "io_uring"
                    }

                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_bias_modal.set(true),
                        "Bias"
                    }

                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_threads_modal.set(true),
                        "Threads"
                    }

                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_entities_production_modal.set(true),
                        "Entities Production"
                    }
                }
            }
        }

        // Embeddings Modal
        if show_embeddings_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",

                div { class: "p-6 max-w-4xl mx-auto pb-20",

                    // Close button
                    div { class: "flex justify-between items-start mb-4",
                        h2 { class: "text-2xl font-bold text-white", "What Embeddings Are" }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_embeddings_modal.set(false),
                            "×"
                        }
                    }

                    p { class: "text-lg text-gray-200 mb-4",
                        "An "
                        strong { "embedding" }
                        " is a way of representing complex objects—like words, sentences, images, or even users—as "
                        strong { "dense, fixed-length numerical vectors" }
                        ". The key idea is that "
                        strong { "geometry becomes meaning" }
                        ": distances and directions between vectors correspond to semantic relationships between the original objects."
                    }

                    // Formal definition
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Formal Definition" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Formally, an embedding takes an input object x and maps it to a vector:"
                    }
                    div { class: "bg-gray-700 rounded p-4 my-4 text-center",
                        code { class: "text-lg text-blue-300", "v ∈ ℝⁿ" }
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "This expression captures several important ideas:"
                    }
                    ul { class: "space-y-2 text-sm text-gray-300 ml-4 list-disc",
                        li {
                            "The symbol "
                            strong { "∈" }
                            " means \"is an element of\" or \"belongs to.\""
                        }
                        li {
                            strong { "ℝ" }
                            " is the set of all real numbers."
                        }
                        li {
                            strong { "ℝⁿ" }
                            " is the set of all n-dimensional vectors of real numbers."
                        }
                    }
                    p { class: "text-sm text-gray-300 mt-4 mb-2",
                        "So the statement "
                        strong { "v ∈ ℝⁿ" }
                        " means:"
                    }
                    div { class: "bg-gray-700 rounded p-4 my-4 border-l-4 border-blue-500",
                        p { class: "text-gray-200 italic",
                            "The embedding v is a vector with n real-valued components, living in an n-dimensional real vector space."
                        }
                    }
                    p { class: "text-sm text-gray-300 mb-2", "A vector in this space looks like:" }
                    div { class: "bg-gray-700 rounded p-4 my-4 text-center",
                        code { class: "text-lg text-blue-300", "v = (v₁, v₂, ..., vₙ)" }
                    }
                    p { class: "text-sm text-gray-300",
                        "Each vᵢ is a real number. Every embedding produced by a given model has the same dimensionality n, which ensures that all embeddings live in a shared geometric space where comparisons make sense."
                    }

                    // Why This Matters
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "Why This Matters" }
                    p { class: "text-sm text-gray-300 mb-4",
                        "Because embeddings live in a structured vector space:"
                    }
                    ul { class: "space-y-3 text-sm text-gray-300 ml-4",
                        li {
                            strong { "Distances" }
                            " reflect similarity (closer vectors → more similar meanings)"
                        }
                        li {
                            strong { "Directions" }
                            " encode relationships (e.g., the famous analogy: king - man + woman ≈ queen)"
                        }
                        li {
                            strong { "Clustering" }
                            " groups related concepts"
                        }
                        li {
                            strong { "Search" }
                            " becomes geometric nearest-neighbor lookup"
                        }
                        li {
                            strong { "Classification" }
                            " becomes linear separation in high-dimensional space"
                        }
                    }
                    p { class: "text-sm text-gray-300 mt-4",
                        "This is why embeddings are so powerful: they turn messy, symbolic, human-level concepts into "
                        strong { "mathematically structured objects" }
                        " that models can reason about."
                    }

                    // How ONNX and Neo4j Relate Through Embeddings
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3",
                        "How ONNX and Neo4j Relate Through Embeddings"
                    }

                    // 1. ONNX
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "1. ONNX: The Model Runtime That Produces Embeddings"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "ONNX (Open Neural Network Exchange) is a model format + runtime. You use it to run models locally or in production without being tied to a specific framework."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Typical workflow:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Load an ONNX model (e.g., a sentence transformer, CLIP, MiniLM, etc.)" }
                        li { "Pass input text/image through it" }
                        li { "The model outputs an embedding vector: v ∈ ℝⁿ" }
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "This vector is your semantic representation. So "
                        strong { "ONNX is the embedding generator" }
                        "."
                    }

                    // 2. Neo4j
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "2. Neo4j: The Graph Database That Stores and Queries Embeddings"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Neo4j is a graph database, but it also has:"
                    }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Native vector properties" }
                        li { "Native vector indexes" }
                        li { "Native vector similarity search (cosine, dot, Euclidean)" }
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-2",
                        "This means you can store embeddings directly on nodes:"
                    }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "CREATE (d:Document {{"
                        br {}
                        "  id: \"doc1\","
                        br {}
                        "  text: \"...\","
                        br {}
                        "  embedding: [0.12, -0.87, 0.003, ...]"
                        br {}
                        "}})"
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-2",
                        "And then run vector similarity queries:"
                    }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "MATCH (d:Document)"
                        br {}
                        "WHERE d.embedding IS NOT NULL"
                        br {}
                        "RETURN d, gds.similarity.cosine(d.embedding, $queryEmbedding) AS score"
                        br {}
                        "ORDER BY score DESC LIMIT 5;"
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "So "
                        strong { "Neo4j is the embedding store + search engine + relationship engine" }
                        "."
                    }

                    // 3. How They Work Together
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "3. How They Work Together"
                    }
                    div { class: "space-y-3 text-sm text-gray-300",
                        div {
                            strong { "Step 1 — Generate embeddings with ONNX" }
                            p { class: "ml-4", "Input: text, sentence, image, user profile, etc." }
                            p { class: "ml-4", "Output: vector (v ∈ ℝⁿ)" }
                        }
                        div {
                            strong { "Step 2 — Store embeddings in Neo4j" }
                            p { class: "ml-4",
                                "Attach the vector to a node: Document, User, Product, Concept, etc."
                            }
                        }
                        div {
                            strong { "Step 3 — Query Neo4j using vector similarity" }
                            ul { class: "ml-4 list-disc",
                                li { "Find nearest neighbors" }
                                li { "Rank by similarity" }
                                li { "Combine vector similarity with graph structure" }
                                li { "Mix embeddings with relationships (this is where Neo4j shines)" }
                            }
                        }
                    }

                    // 4. Why This Combination Is Powerful
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "4. Why This Combination Is Powerful"
                    }
                    div { class: "space-y-2 text-sm text-gray-300",
                        p {
                            strong { "A. Semantic search" }
                            " — ONNX generates embeddings → Neo4j finds similar nodes."
                        }
                        p {
                            strong { "B. Knowledge graph + embeddings" }
                            " — Combine symbolic relationships (edges) with semantic similarity (vectors) for hybrid reasoning."
                        }
                        p {
                            strong { "C. Recommendation systems" }
                            " — User embedding → find similar users or items in Neo4j."
                        }
                        p {
                            strong { "D. RAG (Retrieval-Augmented Generation)" }
                            " — ONNX creates embeddings for chunks, Neo4j stores them, query for nearest neighbors, feed results into your LLM. A strong alternative to Pinecone, Weaviate, or FAISS."
                        }
                    }

                    // 5. Architecture Diagram
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "5. Architecture Diagram"
                    }
                    div { class: "bg-gray-700 rounded p-4 my-2 font-mono text-xs text-blue-300 whitespace-pre",
                        "    ┌──────────────┐\n"
                        "    │   ONNX Model  │\n"
                        "    │  (Embedder)   │\n"
                        "    └──────┬───────┘\n"
                        "           │\n"
                        "           ▼\n"
                        "  v ∈ ℝⁿ (embedding)\n"
                        "           │\n"
                        "           ▼\n"
                        "    ┌──────────────┐\n"
                        "    │    Neo4j      │\n"
                        "    │ Graph+Vectors │\n"
                        "    └──────┬───────┘\n"
                        "           │\n"
                        "           ▼\n"
                        "Similarity search + graph reasoning"
                    }

                    // When are embeddings used
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "When Are Embeddings Used?" }
                    ul { class: "space-y-2 text-sm text-gray-300 ml-4 list-decimal",
                        li { "Document indexing - When you upload/add documents to RAG" }
                        li { "Search queries - Every time you search" }
                        li { "RAG retrieval - When the AI answers questions" }
                        li { "Similarity matching - Comparing documents/chunks" }
                        li { "Agent memory storage - When agents store memories" }
                        li { "Agent memory retrieval - When agents recall past interactions" }
                    }

                    button {
                        class: "btn btn-primary btn-sm mt-8 w-full",
                        onclick: move |_| show_embeddings_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // Knowledge Graphs Modal
        if show_knowledge_graphs_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",

                div { class: "p-6 max-w-4xl mx-auto pb-20",

                    // Close button
                    div { class: "flex justify-between items-start mb-4",
                        h2 { class: "text-2xl font-bold text-white", "Knowledge Graphs" }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_knowledge_graphs_modal.set(false),
                            "×"
                        }
                    }

                    // Introduction
                    p { class: "text-lg text-gray-200 mb-4",
                        "A "
                        strong { "knowledge graph" }
                        " is a data structure that uses "
                        strong { "nodes" }
                        " to represent concepts and entities and "
                        strong { "relationships" }
                        " to connect these nodes."
                    }
                    p { class: "text-sm text-gray-300 mb-4",
                        "Knowledge graphs are highly versatile, capable of storing both structured information (such as employee details, task statuses, and company hierarchies) and unstructured information (such as article contents)."
                    }
                    p { class: "text-sm text-gray-300 mb-4",
                        "A knowledge graph draws a clean distinction between concepts and entities, even though both appear as nodes in the graph. Understanding this distinction is essential because it shapes how meaning, inference, and relationships are modeled."
                    }

                    // Concepts
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "Concepts" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Concepts are "
                        strong { "abstract categories or ideas" }
                        ". They are not tied to a single, specific thing in the world."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Examples:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "\"Animal\"" }
                        li { "\"Vehicle\"" }
                        li { "\"Disease\"" }
                        li { "\"Programming language\"" }
                        li { "\"Company\"" }
                    }
                    p { class: "text-sm text-gray-300 mt-3 mb-2",
                        "A concept represents a "
                        strong { "type, class, or category" }
                        ". In a knowledge graph, concepts often sit higher in the hierarchy and help organize meaning."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "You can think of them as:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "General notions" }
                        li { "Semantic categories" }
                        li { "Things that many entities can belong to" }
                    }
                    p { class: "text-sm text-gray-300 mt-3",
                        "Concepts help the graph express "
                        strong { "what something is" }
                        "."
                    }

                    // Entities
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "Entities" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Entities are "
                        strong { "specific, individual things" }
                        " that exist in the real world."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Examples:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "\"Pieter\"" }
                        li { "\"Microsoft\"" }
                        li { "\"Paris\"" }
                        li { "\"Python 3.12\"" }
                        li { "\"The Eiffel Tower\"" }
                    }
                    p { class: "text-sm text-gray-300 mt-3 mb-2",
                        "An entity represents a "
                        strong { "unique instance" }
                        " of a concept. For example:"
                    }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "\"Dog\" → concept" }
                        li { "\"Fido the dog\" → entity" }
                        li { "\"City\" → concept" }
                        li { "\"Paris\" → entity" }
                    }
                    p { class: "text-sm text-gray-300 mt-3",
                        "Entities help the graph express "
                        strong { "which specific thing" }
                        " we are talking about."
                    }

                    // How They Work Together
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3",
                        "How They Work Together in a Knowledge Graph"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "A simple structure might look like this:"
                    }
                    div { class: "bg-gray-700 rounded p-4 my-4 font-mono text-sm text-green-300",
                        "(Paris) —[:IS_A]→ (City)"
                        br {}
                        "(Eiffel Tower) —[:LOCATED_IN]→ (Paris)"
                        br {}
                        "(Python 3.12) —[:IS_A]→ (Programming language)"
                    }
                    p { class: "text-sm text-gray-300 mt-3 mb-2", "Where:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li {
                            strong { "City" }
                            " and "
                            strong { "Programming language" }
                            " are concepts"
                        }
                        li {
                            strong { "Paris" }
                            ", "
                            strong { "Eiffel Tower" }
                            ", and "
                            strong { "Python 3.12" }
                            " are entities"
                        }
                    }
                    div { class: "bg-gray-700 rounded p-4 my-4 border-l-4 border-blue-500",
                        p { class: "text-gray-200",
                            strong { "Concepts" }
                            " give structure."
                            br {}
                            strong { "Entities" }
                            " give concrete meaning."
                            br {}
                            strong { "Relationships" }
                            " connect them into a navigable semantic network."
                        }
                    }

                    // How This Relates to Embeddings
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3",
                        "How This Relates to Embeddings"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Embeddings give you a vector representation: "
                        strong { "v ∈ ℝⁿ" }
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Meaning:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "∈ means \"is an element of\"" }
                        li { "The embedding vector (v) belongs to an n-dimensional real vector space" }
                        li { "Every concept and entity can have its own embedding" }
                    }
                    p { class: "text-sm text-gray-300 mt-3 mb-2",
                        "This is where things get interesting:"
                    }
                    div { class: "space-y-3 text-sm text-gray-300",
                        div {
                            strong { "Concepts get concept embeddings" }
                            p { class: "ml-4", "These capture the general meaning of a category." }
                            p { class: "ml-4 text-gray-400",
                                "Example: The embedding for \"City\" encodes the idea of cities in general."
                            }
                        }
                        div {
                            strong { "Entities get entity embeddings" }
                            p { class: "ml-4",
                                "These capture the specific meaning of a particular instance."
                            }
                            p { class: "ml-4 text-gray-400",
                                "Example: The embedding for \"Paris\" encodes the specific city, not the general idea of cities."
                            }
                        }
                    }
                    p { class: "text-sm text-gray-300 mt-3",
                        "Because embeddings live in the same vector space, you can compare: Entity ↔ Entity, Entity ↔ Concept, Concept ↔ Concept. This lets you do semantic reasoning on top of your graph."
                    }

                    // How ONNX Fits In
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "How ONNX Fits In" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "ONNX is the runtime that generates embeddings."
                    }
                    p { class: "text-sm text-gray-300 mb-1", "You feed it:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "text, sentences, documents" }
                        li { "images" }
                        li { "node labels, descriptions" }
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-1",
                        "It outputs: a vector (v ∈ ℝⁿ)"
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-1", "You can generate embeddings for:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Concepts (\"City\", \"Programming language\")" }
                        li { "Entities (\"Paris\", \"Python 3.12\")" }
                        li { "Relationships (\"located in\", \"is a\")" }
                        li { "Whole subgraphs (if you encode them)" }
                    }

                    // How Neo4j Fits In
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "How Neo4j Fits In" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Neo4j is the graph database that stores and queries embeddings."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "You attach embeddings to nodes:" }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "(:Concept {{name: \"City\", embedding: [...]}})"
                        br {}
                        "(:Entity  {{name: \"Paris\", embedding: [...]}})"
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-1", "Neo4j can then:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Perform vector similarity search" }
                        li { "Combine vector similarity with graph structure" }
                        li { "Infer relationships" }
                        li { "Cluster similar nodes" }
                        li { "Support RAG pipelines" }
                        li { "Build hybrid symbolic + semantic reasoning systems" }
                    }

                    // Putting It All Together
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "Putting It All Together" }
                    p { class: "text-sm text-gray-300 mb-2", "Here's the full pipeline:" }
                    div { class: "bg-gray-700 rounded p-4 my-2 font-mono text-xs text-blue-300 whitespace-pre",
                        "      ONNX Model\n"
                        "  (Embedding Generator)\n"
                        "          │\n"
                        "          ▼\n"
                        "  v ∈ ℝⁿ (embedding)\n"
                        "          │\n"
                        "          ▼\n"
                        "      Neo4j Graph\n"
                        "(Concepts + Entities + Vectors)\n"
                        "          │\n"
                        "          ▼\n"
                        "Semantic search + graph reasoning"
                    }
                    p { class: "text-sm text-gray-300 mt-4 mb-2", "Example query:" }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "MATCH (e:Entity)"
                        br {}
                        "RETURN e, gds.similarity.cosine(e.embedding, $queryEmbedding) AS score"
                        br {}
                        "ORDER BY score DESC LIMIT 5"
                    }
                    p { class: "text-sm text-gray-300 mt-3", "You can now:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Find cities similar to Paris" }
                        li { "Find entities similar to a concept" }
                        li {
                            "Build hybrid reasoning systems that mix symbolic edges with semantic vectors"
                        }
                    }

                    button {
                        class: "btn btn-primary btn-sm mt-8 w-full",
                        onclick: move |_| show_knowledge_graphs_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // ONNX Modal
        if show_onnx_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",

                div { class: "p-6 max-w-6xl mx-auto pb-20",

                    // Close button
                    div { class: "flex justify-between items-start mb-4",
                        h2 { class: "text-2xl font-bold text-white",
                            "ONNX - Open Neural Network Exchange"
                        }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_onnx_modal.set(false),
                            "×"
                        }
                    }

                    p { class: "text-lg text-gray-200 mb-6",
                        "ONNX is a universal file format for ML models. Train anywhere, run anywhere."
                    }

                    // The Problem It Solves
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "The Problem It Solves" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Before ONNX, a PyTorch model only ran in PyTorch. A TensorFlow model only ran in TensorFlow. Want to run a PyTorch model on mobile? Rewrite it. Want to run a TensorFlow model in Rust? Good luck. Want to optimize for Intel, NVIDIA, or AMD? Different format for each."
                    }
                    p { class: "text-sm text-gray-300",
                        "After ONNX, you export from PyTorch, TensorFlow, Keras, or JAX into one .onnx file. That file runs anywhere: ONNX Runtime, TensorRT, OpenVINO, CoreML, DirectML, WebNN."
                    }

                    // What's Inside an ONNX File
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "What's Inside an ONNX File" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "An ONNX file contains three main parts."
                    }
                    p { class: "text-sm text-gray-300 mb-1",
                        "First, the graph. This defines the computation structure: what operations to perform (MatMul, Add, ReLU, Softmax) and how data flows between them. It also specifies input and output tensor shapes and types."
                    }
                    p { class: "text-sm text-gray-300 mb-1",
                        "Second, the weights. These are the learned parameters: layer weights and biases stored as tensors with their shapes and data types."
                    }
                    p { class: "text-sm text-gray-300 mb-1",
                        "Third, metadata. This includes the opset version (which operators are available), the producer (like \"pytorch 2.0\"), and the model version."
                    }
                    p { class: "text-sm text-gray-300",
                        "Optionally, it may include quantization information."
                    }

                    // ONNX Operators
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX Operators" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "ONNX defines standard building blocks that all runtimes understand."
                    }
                    p { class: "text-sm text-gray-300",
                        "Math operators include Add, Sub, Mul, Div, MatMul, Gemm, Sqrt, Exp, and Log."
                    }
                    p { class: "text-sm text-gray-300",
                        "Activation operators include Relu, Sigmoid, Tanh, Softmax, Gelu, and Silu."
                    }
                    p { class: "text-sm text-gray-300",
                        "Tensor operators include Reshape, Transpose, Concat, Split, Slice, and Squeeze."
                    }
                    p { class: "text-sm text-gray-300",
                        "Reduction operators include ReduceSum, ReduceMean, and ReduceMax."
                    }
                    p { class: "text-sm text-gray-300",
                        "Normalization operators include BatchNorm, LayerNorm, and InstanceNorm."
                    }
                    p { class: "text-sm text-gray-300",
                        "Convolution operators include Conv, ConvTranspose, MaxPool, and AveragePool."
                    }
                    p { class: "text-sm text-gray-300",
                        "Quantization operators include QuantizeLinear and DequantizeLinear."
                    }
                    p { class: "text-sm text-gray-400 mt-2",
                        "The opset version determines which operators are available. Higher versions have more features."
                    }

                    // ONNX vs Other Formats
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX vs Other Formats" }
                    p { class: "text-sm text-gray-300",
                        "ONNX was created by Microsoft and Meta. It's best for cross-platform inference and has the highest portability across platforms."
                    }
                    p { class: "text-sm text-gray-300",
                        "GGUF was created by llama.cpp. It's best for LLM text generation but only works with llama.cpp and its derivatives."
                    }
                    p { class: "text-sm text-gray-300",
                        "SafeTensors was created by HuggingFace. It's best for weight storage and has good portability."
                    }
                    p { class: "text-sm text-gray-300",
                        "TorchScript was created by PyTorch. It's best for PyTorch deployment but has limited portability outside PyTorch."
                    }
                    p { class: "text-sm text-gray-300",
                        "SavedModel was created by TensorFlow. It's best for TensorFlow deployment but has limited portability outside TensorFlow."
                    }
                    p { class: "text-sm text-gray-300",
                        "TensorRT was created by NVIDIA. It's best for NVIDIA GPU inference but only works on NVIDIA hardware."
                    }
                    p { class: "text-sm text-gray-300",
                        "CoreML was created by Apple. It's best for Apple devices but only works on Apple hardware."
                    }

                    // Creating ONNX Models
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Creating ONNX Models" }
                    p { class: "text-sm text-gray-300",
                        "From PyTorch, you use torch.onnx.export. Pass your model, a dummy input tensor, the output filename, input names, output names, and optionally dynamic axes for variable batch sizes."
                    }
                    p { class: "text-sm text-gray-300",
                        "From TensorFlow, you use tf2onnx.convert.from_keras. Pass your loaded Keras model and the output path."
                    }
                    p { class: "text-sm text-gray-300",
                        "From HuggingFace, the easiest way is the optimum CLI. Install optimum with onnxruntime, then run optimum-cli export onnx with your model name and output directory."
                    }

                    // Running ONNX Models
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Running ONNX Models" }
                    p { class: "text-sm text-gray-300",
                        "In Python, you create an InferenceSession with onnxruntime, passing the model path. Then call session.run with None for output names and a dictionary of inputs."
                    }
                    p { class: "text-sm text-gray-300",
                        "In Rust, you use the ort crate. Build a Session from the model file, then call session.run with your input tensors."
                    }
                    p { class: "text-sm text-gray-300",
                        "In C++, you create an Ort::Session with the environment, model path, and session options. Then call session.Run with run options, input names, input tensors, and output names."
                    }

                    // ONNX Runtime Execution Providers
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "ONNX Runtime Execution Providers"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "ONNX Runtime can run the same model file on different hardware through execution providers."
                    }
                    p { class: "text-sm text-gray-300", "CPU is the default and works everywhere." }
                    p { class: "text-sm text-gray-300", "CUDA runs on NVIDIA GPUs." }
                    p { class: "text-sm text-gray-300", "TensorRT runs optimized on NVIDIA GPUs." }
                    p { class: "text-sm text-gray-300", "ROCm runs on AMD GPUs." }
                    p { class: "text-sm text-gray-300", "OpenVINO runs optimized on Intel hardware." }
                    p { class: "text-sm text-gray-300", "DirectML runs on Windows GPUs." }
                    p { class: "text-sm text-gray-300", "CoreML runs on Apple hardware." }
                    p { class: "text-sm text-gray-300", "NNAPI runs on Android." }
                    p { class: "text-sm text-gray-300", "WebNN runs in browsers." }
                    p { class: "text-sm text-gray-400 mt-2",
                        "Same model file, different hardware acceleration."
                    }

                    // ONNX Optimizations
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX Optimizations" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "ONNX Runtime optimizes the graph before running."
                    }
                    p { class: "text-sm text-gray-300",
                        "It fuses operations. A sequence of MatMul, Add, and Relu becomes a single FusedMatMulAddRelu operation."
                    }
                    p { class: "text-sm text-gray-300",
                        "It folds constants. Conv followed by BatchNorm gets folded into modified weights."
                    }
                    p { class: "text-sm text-gray-300",
                        "It removes redundant operations like unnecessary Cast ops."
                    }
                    p { class: "text-sm text-gray-300",
                        "It combines multiple Transpose operations into one."
                    }
                    p { class: "text-sm text-gray-400 mt-2",
                        "You control optimization level when creating the session. Level 3 or ORT_ENABLE_ALL gives maximum optimization."
                    }

                    // Quantization
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Quantization" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Quantization shrinks models and speeds them up."
                    }
                    p { class: "text-sm text-gray-300",
                        "An original FP32 model might be 400 MB and slower. After quantization to INT8, it becomes 100 MB and runs 2-4x faster."
                    }
                    p { class: "text-sm text-gray-300",
                        "You use onnxruntime.quantization.quantize_dynamic in Python. Pass the input model path, output model path, and weight type like QInt8."
                    }

                    // For Your AG Project
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "For Your AG Project" }
                    div { class: "bg-gray-700 rounded p-4 text-sm text-gray-200",
                        p { class: "mb-2",
                            "The model is embedding_model.onnx. It runs through the ort crate and produces vectors."
                        }
                        p { class: "mb-3",
                            "You use GGUF for your LLM. The model is something like phi-3.gguf. It runs through Ollama or llama.cpp and produces text output."
                        }
                        p { class: "text-xs text-gray-400 mt-2",
                            "Why ONNX for embeddings? It's a single forward pass with no autoregressive loop. It has fast inference. The models are small at 50-400 MB. The ort crate makes Rust integration easy."
                        }
                        p { class: "text-xs text-gray-400",
                            "Why GGUF for the LLM? It's optimized for token-by-token generation. It has better quantization for large models. It handles KV cache management. It has the whole llama.cpp ecosystem."
                        }
                    }

                    // Summary
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Summary" }
                    p { class: "text-sm text-gray-300",
                        "ONNX stands for Open Neural Network Exchange."
                    }
                    p { class: "text-sm text-gray-300", "Microsoft and Meta created it." }
                    p { class: "text-sm text-gray-300", "The file extension is .onnx." }
                    p { class: "text-sm text-gray-300",
                        "The main benefit is train anywhere, run anywhere."
                    }
                    p { class: "text-sm text-gray-300", "The Rust crate is ort." }
                    p { class: "text-sm text-gray-300",
                        "It's best for inference, embeddings, and cross-platform deployment."
                    }

                    button {
                        class: "btn btn-primary btn-sm mt-6 w-full",
                        onclick: move |_| show_onnx_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // ONNX Parameters Modal
        if show_onnx_params_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",

                div { class: "p-6 max-w-6xl mx-auto pb-20",

                    div { class: "flex justify-between items-start mb-4",
                        h2 { class: "text-2xl font-bold text-white", "ONNX Parameters" }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_onnx_params_modal.set(false),
                            "×"
                        }
                    }

                    p { class: "text-lg text-gray-200 mb-6",
                        "Environment variables and configuration for ONNX in your AG project."
                    }

                    // Required Environment Variables
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "Required Environment Variables"
                    }
                    div { class: "bg-gray-700 rounded p-4 font-mono text-sm text-gray-200 space-y-2",
                        p { "export ORT_DYLIB_PATH=/usr/local/lib/libonnxruntime.so" }
                        p { "export ONNX_MODEL_PATH=models/embedding_model.onnx" }
                    }

                    // ORT_DYLIB_PATH
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ORT_DYLIB_PATH" }
                    p { class: "text-sm text-gray-300",
                        "Path to the ONNX Runtime shared library. Required for the ort crate to load the runtime."
                    }
                    p { class: "text-sm text-gray-300 mt-2", "Common locations:" }
                    p { class: "text-sm text-gray-400 font-mono",
                        "Linux: /usr/local/lib/libonnxruntime.so"
                    }
                    p { class: "text-sm text-gray-400 font-mono",
                        "macOS: /usr/local/lib/libonnxruntime.dylib"
                    }
                    p { class: "text-sm text-gray-400 font-mono",
                        "Windows: C:\\onnxruntime\\lib\\onnxruntime.dll"
                    }

                    // ONNX_MODEL_PATH
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX_MODEL_PATH" }
                    p { class: "text-sm text-gray-300",
                        "Path to your ONNX embedding model file. Defaults to models/embedding_model.onnx if not set."
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "Your current model: models/embedding_model.onnx"
                    }

                    // Runtime Library
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX Runtime Library" }
                    p { class: "text-sm text-gray-300", "Location: /usr/local/lib/" }
                    p { class: "text-sm text-gray-300", "Version: 1.20.1" }
                    p { class: "text-sm text-gray-300", "Files:" }
                    p { class: "text-sm text-gray-400 font-mono ml-4", "libonnxruntime.so.1.20.1" }
                    p { class: "text-sm text-gray-400 font-mono ml-4",
                        "libonnxruntime.so.1 → libonnxruntime.so.1.20.1"
                    }
                    p { class: "text-sm text-gray-400 font-mono ml-4",
                        "libonnxruntime.so → libonnxruntime.so.1"
                    }
                    p { class: "text-sm text-gray-400 font-mono ml-4",
                        "libonnxruntime_providers_shared.so"
                    }

                    // Rust Crate
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Rust Integration" }
                    p { class: "text-sm text-gray-300", "Crate: ort" }
                    p { class: "text-sm text-gray-300", "Feature flag: --features onnx" }
                    p { class: "text-sm text-gray-300 mt-2", "Build command:" }
                    p { class: "text-sm text-gray-400 font-mono", "cargo run --features onnx" }

                    button {
                        class: "btn btn-primary btn-sm mt-6 w-full",
                        onclick: move |_| show_onnx_params_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // io_uring Modal
        if show_io_uring_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",

                div { class: "px-4 py-4 w-full pb-20",

                    // Close button
                    div { class: "flex justify-between items-start mb-2",
                        h2 { class: "text-xl font-bold text-white",
                            "io_uring: A Unified Async I/O API for Linux"
                        }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_io_uring_modal.set(false),
                            "✕"
                        }
                    }

                    // Row 1: What is io_uring + Problem/Solution diagrams side by side
                    div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4 mb-4",
                        // What is io_uring?
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "What is io_uring?" }
                            p { class: "text-xs text-gray-300 mb-2",
                                "Linux kernel interface (5.1+) for async I/O:"
                            }
                            ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-0.5",
                                li { "One API for all I/O types" }
                                li { "Zero/minimal syscalls" }
                                li { "True async (not thread pools)" }
                                li { "Batching of operations" }
                            }
                            p { class: "text-xs text-yellow-300 mt-2",
                                "⭐ File I/O (doc ingestion, index loading) is where io_uring helps most!"
                            }
                        }

                        // Before io_uring
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Before (Fragmented)" }
                            pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                                "Files:   AIO         - Limited
Sockets: epoll       - Different API
Timers:  timerfd     - Yet another
Signals: signalfd    - And another
Events:  eventfd     - More APIs

❌ Each I/O = different API
❌ Can't batch mixed ops"
                            }
                        }

                        // With io_uring
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "With io_uring (Unified)" }
                            pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                                "Files ─────────┐
Sockets ───────┤ io_uring ─► CQ
Timers ────────┤ (One API)
Signals ───────┘

✅ One API for everything
✅ Batch N ops in 1 syscall
✅ True kernel-level async"
                            }
                        }
                    }

                    // Row 2: Architecture + Performance side by side
                    div { class: "grid grid-cols-1 lg:grid-cols-2 gap-4 mb-4",
                        // Architecture
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Architecture" }
                            pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                                "USER SPACE              KERNEL SPACE

┌───────────────┐        ┌─────────────┐
│ Submission Q │◄─shared─►│  io_uring   │
│     (SQ)     │ memory  │   kernel    │
└──────┬────────┘        └──────┬──────┘
       │ submit                  │
       ▼                         │ complete
┌───────────────┐                 │
│ Completion Q │◄─────────────────┘
│     (CQ)     │ shared memory
└───────────────┘

Key: Shared rings = no copy, minimal syscalls"
                            }
                        }

                        // Performance + TL;DR
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Performance" }
                            table { class: "table table-xs text-gray-300 w-full",
                                thead {
                                    tr {
                                        th { class: "text-gray-200 text-xs", "Metric" }
                                        th { class: "text-gray-200 text-xs", "epoll" }
                                        th { class: "text-gray-200 text-xs", "io_uring" }
                                    }
                                }
                                tbody {
                                    tr {
                                        td { "Syscalls/IO" }
                                        td { "1-2" }
                                        td { class: "text-green-400", "0-1" }
                                    }
                                    tr {
                                        td { "File async" }
                                        td { class: "text-red-400", "Fake" }
                                        td { class: "text-green-400", "True" }
                                    }
                                    tr {
                                        td { "Batching" }
                                        td { class: "text-red-400", "No" }
                                        td { class: "text-green-400", "Yes" }
                                    }
                                    tr {
                                        td { "Zero-copy" }
                                        td { class: "text-red-400", "Limited" }
                                        td { class: "text-green-400", "Yes" }
                                    }
                                    tr {
                                        td { "CPU" }
                                        td { "Higher" }
                                        td { class: "text-green-400", "30-50% lower" }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-400 mt-2",
                                "Benchmark: epoll ~400k ops/s → io_uring ~800k ops/s (2x)"
                            }
                        }
                    }

                    // Code Comparison - side by side
                    h3 { class: "text-lg font-bold text-white mb-2", "Code Comparison" }
                    div { class: "grid grid-cols-1 lg:grid-cols-2 gap-4 mb-4",
                        div { class: "bg-gray-900 rounded p-3",
                            p { class: "text-xs text-red-400 font-bold mb-1", "epoll (Old Way)" }
                            pre { class: "text-[10px] text-green-300 font-mono leading-tight",
                                "let epoll_fd = epoll_create1(0)?;

// Files need thread pool!
let data = spawn_blocking(|| {{
    std::fs::read(\"data.bin\")
}}).await?;

// Socket = different API
let socket = TcpStream::connect(addr)?;
socket.set_nonblocking(true)?;
epoll_ctl(epoll_fd, ADD, fd, &ev)?;

epoll_wait(epoll_fd, &mut evs)?; // syscall!
socket.read(&mut buf)?;          // syscall!"
                            }
                        }
                        div { class: "bg-gray-900 rounded p-3",
                            p { class: "text-xs text-green-400 font-bold mb-1", "io_uring (New Way)" }
                            pre { class: "text-[10px] text-green-300 font-mono leading-tight",
                                "let mut ring = IoUring::new(256)?;

// Queue file (no syscall yet!)
let read_e = opcode::Read::new(fd, buf)
    .build().user_data(0x01);

// Queue socket (same API!)
let recv_e = opcode::Recv::new(fd, buf)
    .build().user_data(0x02);

// Submit ALL at once!
ring.submission()
    .push(&read_e)?.push(&recv_e)?;
ring.submit()?; // 1 syscall for N ops!"
                            }
                        }
                    }

                    // Row 3: Supported Operations + Rust Ecosystem + AG Project side by side
                    div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4 mb-4",
                        // Supported Operations
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Supported Operations" }
                            div { class: "text-[10px] text-gray-300 space-y-0.5",
                                p {
                                    span { class: "text-gray-400", "File: " }
                                    "Read, Write, Fsync, Fallocate"
                                }
                                p {
                                    span { class: "text-gray-400", "Socket: " }
                                    "Accept, Connect, Recv, Send"
                                }
                                p {
                                    span { class: "text-gray-400", "Advanced: " }
                                    "SendZc, RecvMulti"
                                }
                                p {
                                    span { class: "text-gray-400", "Timers: " }
                                    "Timeout, LinkTimeout"
                                }
                                p {
                                    span { class: "text-gray-400", "Polling: " }
                                    "PollAdd, PollRemove"
                                }
                                p {
                                    span { class: "text-gray-400", "Files: " }
                                    "Open, Close, Stat, Rename"
                                }
                                p {
                                    span { class: "text-gray-400", "Misc: " }
                                    "Cancel, Splice, Shutdown"
                                }
                            }
                            p { class: "text-xs text-green-400 mt-2", "All through ONE API!" }
                        }

                        // Rust Ecosystem
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Rust Ecosystem" }
                            div { class: "text-[10px] text-gray-300 space-y-0.5",
                                p {
                                    span { class: "font-mono text-blue-300", "io-uring" }
                                    " - Low-level bindings"
                                }
                                p {
                                    span { class: "font-mono text-blue-300", "tokio-uring" }
                                    " - Tokio + io_uring"
                                }
                                p {
                                    span { class: "font-mono text-blue-300", "glommio" }
                                    " - Thread-per-core"
                                }
                                p {
                                    span { class: "font-mono text-blue-300", "monoio" }
                                    " - High-perf runtime"
                                }
                                p {
                                    span { class: "font-mono text-blue-300", "nuclei" }
                                    " - Proactive I/O"
                                }
                            }
                            pre { class: "text-[9px] text-green-300 font-mono mt-2 leading-tight",
                                "#[tokio_uring::main]
async fn main() {{
  let file = File::open(\"x\").await?;
  let (res, buf) = file.read_at(buf, 0).await;
}}"
                            }
                        }

                        // AG Project Relevance
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "For AG Project" }
                            div { class: "text-[10px] text-gray-300 space-y-0.5",
                                p {
                                    span { class: "text-gray-400", "Vector index: " }
                                    "True async file reads"
                                }
                                p {
                                    span { class: "text-gray-400", "Ingestion: " }
                                    "Batch file operations"
                                }
                                p {
                                    span { class: "text-gray-400", "HTTP API: " }
                                    "Lower latency sockets"
                                }
                                p {
                                    span { class: "text-gray-400", "Redis L3: " }
                                    "Faster network I/O"
                                }
                                p {
                                    span { class: "text-gray-400", "Logging: " }
                                    "Efficient file writes"
                                }
                            }
                            p { class: "text-[10px] text-green-400 mt-2",
                                "✅ Hybrid mode: Actix+Tokio for HTTP, io_uring for file I/O (vectors, cache). Enable with --features io_uring"
                            }
                        }
                    }

                    // Close button at bottom
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_io_uring_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // Bias Prejudice Modal
        if show_bias_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",

                div { class: "p-6 max-w-4xl mx-auto pb-20",

                    // Close button
                    div { class: "flex justify-between items-start mb-4",
                        h2 { class: "text-2xl font-bold text-white", "Bias: Two Different Meanings" }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_bias_modal.set(false),
                            "×"
                        }
                    }

                    p { class: "text-lg text-gray-200 mb-6",
                        "The word 'bias' means completely different things in ML depending on context."
                    }

                    // Social/Cognitive Bias
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "1. Social/Cognitive Bias (Training Data, Behavior)"
                    }
                    div { class: "bg-red-900/30 border border-red-700 rounded p-4 mb-4",
                        p { class: "text-sm text-gray-200 mb-3",
                            "What people usually mean in public discourse—the model favoring certain viewpoints, perpetuating stereotypes, having political leanings, etc."
                        }
                        p { class: "text-sm text-gray-300 mb-2", "This comes from:" }
                        ul { class: "text-sm text-gray-300 list-disc ml-6 space-y-1",
                            li { "Training data reflecting societal biases" }
                            li { "RLHF (Reinforcement Learning from Human Feedback) choices" }
                            li { "Imbalanced representation in datasets" }
                            li { "Human annotator biases" }
                        }
                    }

                    // Mathematical Bias
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "2. Mathematical Bias (Weight Parameter)"
                    }
                    div { class: "bg-blue-900/30 border border-blue-700 rounded p-4 mb-4",
                        p { class: "text-sm text-gray-200 mb-3",
                            "ML papers and code are full of \"bias terms\", \"bias vectors\", \"bias parameters\". This is a completely unrelated concept."
                        }
                        p { class: "text-sm text-gray-300 mb-2",
                            "In a neural network: y = σ(W·x + b)"
                        }
                        p { class: "text-sm text-gray-300",
                            "The 'b' is the bias—a learnable offset that shifts the activation function. It allows neurons to activate even when inputs are zero."
                        }
                    }

                    // The Naming Problem
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "The Naming Problem" }
                    div { class: "bg-gray-900 rounded p-4",
                        p { class: "text-sm text-gray-300 mb-3",
                            "These are completely unrelated concepts that unfortunately share a name."
                        }
                        p { class: "text-sm text-gray-300 mb-3",
                            "The mathematical term predates the fairness discussion by decades—it comes from statistics (as in \"bias-variance tradeoff\")."
                        }
                        p { class: "text-sm text-yellow-300",
                            "Some people have suggested renaming the mathematical one to \"offset\" in educational materials, but the terminology is deeply entrenched."
                        }
                    }

                    // Quick Reference
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Quick Reference" }
                    table { class: "table table-sm w-full text-gray-300",
                        thead {
                            tr {
                                th { class: "text-gray-200", "" }
                                th { class: "text-gray-200", "Social Bias" }
                                th { class: "text-gray-200", "Mathematical Bias" }
                            }
                        }
                        tbody {
                            tr {
                                td { "Meaning" }
                                td { "Prejudice, unfairness" }
                                td { "Numeric offset" }
                            }
                            tr {
                                td { "Source" }
                                td { "Training data, RLHF" }
                                td { "Learned parameter" }
                            }
                            tr {
                                td { "Fix" }
                                td { "Better data, alignment" }
                                td { "N/A (it's intentional)" }
                            }
                            tr {
                                td { "In code" }
                                td { "Not visible" }
                                td { "model.bias, b tensor" }
                            }
                            tr {
                                td { "Origin" }
                                td { "Fairness research" }
                                td { "Statistics (1900s)" }
                            }
                        }
                    }

                    // Summary
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Summary" }
                    div { class: "bg-gray-700 rounded p-4 text-sm text-gray-200",
                        p { class: "mb-2",
                            "• When someone says \"the model is biased\" → they mean social/cognitive bias (prejudice)"
                        }
                        p { class: "mb-2",
                            "• When code says \"bias=True\" or \"self.bias\" → it means the mathematical offset parameter"
                        }
                        p { "• Context is everything—same word, completely different meanings" }
                    }

                    button {
                        class: "btn btn-primary btn-sm mt-6 w-full",
                        onclick: move |_| show_bias_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // Threads Modal
        if show_threads_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",

                div { class: "p-6 max-w-5xl mx-auto pb-20",

                    // Close button
                    div { class: "flex justify-between items-start mb-4",
                        h2 { class: "text-2xl font-bold text-white",
                            "Threads in Rust: Tokio, Rayon & spawn_blocking"
                        }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_threads_modal.set(false),
                            "×"
                        }
                    }

                    // What is a Thread?
                    div { class: "bg-gray-900 rounded p-4 mb-6",
                        div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                            // Left: Definition
                            div {
                                p { class: "text-sm text-gray-200 mb-3",
                                    "A thread is the smallest unit of execution that a CPU can schedule and run. It represents a single sequence of instructions inside a program."
                                }
                                ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-1",
                                    li { "Threads are managed independently by the OS scheduler" }
                                    li {
                                        "Multiple threads in the same process share memory and resources"
                                    }
                                    li {
                                        "Opening an app creates at least one thread; more appear as tasks run concurrently"
                                    }
                                    li {
                                        "Modern systems support multithreading for better responsiveness and performance"
                                    }
                                }
                            }
                            // Right: Why Threads Matter
                            div {
                                h4 { class: "text-sm font-bold text-white mb-2",
                                    "🧠 Why Threads Matter"
                                }
                                p { class: "text-xs text-gray-300 mb-2", "Threads allow programs to:" }
                                ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-1",
                                    li {
                                        "Perform multiple tasks at once (UI stays responsive while work happens in background)"
                                    }
                                    li { "Use multiple CPU cores efficiently" }
                                    li { "Handle I/O without blocking the entire program" }
                                    li { "Scale workloads in servers, game engines, data pipelines" }
                                }
                            }
                        }
                    }

                    // Async from Thread Perspective
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "🧵 Async: A Thread-Saving Mechanism"
                    }
                    div { class: "bg-gray-900 rounded p-4 mb-6",
                        div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                            // Left: Why Async Exists
                            div {
                                p { class: "text-sm text-yellow-300 font-semibold mb-2",
                                    "Threads are powerful but expensive"
                                }
                                ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-1 mb-3",
                                    li { "Large stacks (2-8 MB each)" }
                                    li { "Kernel scheduling overhead" }
                                    li { "Context switching cost" }
                                    li { "10k threads is already painful" }
                                }
                                p { class: "text-sm text-blue-300 font-semibold mb-2",
                                    "I/O-bound threads are wasteful"
                                }
                                p { class: "text-xs text-gray-300",
                                    "A thread waiting on a socket, file, timer, or DB call is mostly idle. Async solves: how to handle millions of I/O ops without millions of threads?"
                                }
                            }
                            // Right: What Async Is
                            div {
                                p { class: "text-sm text-green-300 font-semibold mb-2",
                                    "Async tasks are NOT threads"
                                }
                                ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-1 mb-3",
                                    li { "Not scheduled by the OS" }
                                    li { "Not running in parallel" }
                                    li { "Not preemptive" }
                                }
                                p { class: "text-xs text-gray-300 mb-2",
                                    "An async task is a tiny state machine, polled by an executor, suspended when waiting, resumed when ready."
                                }
                                div { class: "bg-gray-800 rounded p-2 text-xs",
                                    p { class: "text-purple-300", "Thread = a worker" }
                                    p { class: "text-blue-300", "Async task = a to-do list entry" }
                                    p { class: "text-gray-400", "Executor = worker checking the list" }
                                }
                            }
                        }
                        // When to use which
                        div { class: "mt-4 pt-4 border-t border-gray-700",
                            div { class: "grid grid-cols-1 md:grid-cols-2 gap-4 text-xs",
                                div {
                                    p { class: "text-purple-300 font-semibold mb-1",
                                        "Use THREADS when:"
                                    }
                                    p { class: "text-gray-300",
                                        "CPU-bound, true parallelism, heavy computation, independent stacks"
                                    }
                                }
                                div {
                                    p { class: "text-blue-300 font-semibold mb-1",
                                        "Use ASYNC when:"
                                    }
                                    p { class: "text-gray-300",
                                        "I/O-bound, 10k-1M connections, minimal memory, cooperative scheduling"
                                    }
                                }
                            }
                        }
                    }

                    // What Makes Rust Special
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "🦀 What Makes Rust Special"
                    }
                    div { class: "bg-orange-900/20 border border-orange-700 rounded p-4 mb-6",
                        div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                            // Left: Ownership & Threads
                            div {
                                p { class: "text-sm text-orange-300 font-semibold mb-2",
                                    "Fearless Concurrency"
                                }
                                p { class: "text-xs text-gray-300 mb-2",
                                    "Rust's ownership system prevents data races at compile time:"
                                }
                                ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-1",
                                    li {
                                        span { class: "text-green-300", "Send" }
                                        " – type can be transferred to another thread"
                                    }
                                    li {
                                        span { class: "text-green-300", "Sync" }
                                        " – type can be shared between threads"
                                    }
                                    li { "Compiler enforces these automatically" }
                                    li { "No runtime cost – all checked at compile time" }
                                }
                                p { class: "text-xs text-gray-400 mt-2",
                                    "Other languages: data races are runtime bugs. Rust: they don't compile."
                                }
                            }
                            // Right: Why Rust Async is Different
                            div {
                                p { class: "text-sm text-orange-300 font-semibold mb-2",
                                    "Zero-Cost Async"
                                }
                                p { class: "text-xs text-gray-300 mb-2",
                                    "Rust has no GC, no built-in runtime, no scheduler. So async must be:"
                                }
                                ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-1",
                                    li {
                                        span { class: "text-yellow-300", "Zero-cost" }
                                        " – no overhead vs hand-written state machines"
                                    }
                                    li {
                                        span { class: "text-yellow-300", "Explicit" }
                                        " – you choose the executor (Tokio, async-std)"
                                    }
                                    li {
                                        span { class: "text-yellow-300", "State-machine compiled" }
                                        " – async fn → enum at compile time"
                                    }
                                    li {
                                        span { class: "text-yellow-300", "No hidden allocations" }
                                        " – you control memory"
                                    }
                                }
                                p { class: "text-xs text-gray-400 mt-2",
                                    "Go/JS: runtime handles everything. Rust: you're in control."
                                }
                            }
                        }
                        div { class: "mt-4 pt-3 border-t border-orange-700/50 text-xs text-center text-orange-200",
                            "Rust async is not a general concurrency model — it's a "
                            span { class: "font-semibold", "thread-saving mechanism" }
                            " for I/O workloads."
                        }
                    }

                    // Analogy
                    h3 { class: "text-xl font-bold text-white mt-4 mb-3", "🧩 Quick Analogy" }
                    div { class: "bg-gray-700 rounded p-4 mb-6",
                        p { class: "text-sm text-gray-200 mb-2",
                            "Think of a "
                            span { class: "text-blue-300 font-semibold", "process" }
                            " as a workshop."
                        }
                        p { class: "text-sm text-gray-200 mb-3",
                            "A "
                            span { class: "text-green-300 font-semibold", "thread" }
                            " is a worker inside that workshop."
                        }
                        ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-1",
                            li { "All workers share the same tools (memory, code)" }
                            li { "Each worker can perform a different task" }
                            li { "More workers → more tasks done in parallel (if coordinated well)" }
                        }
                    }

                    // Thread vs Task Types
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Threads vs Tasks" }
                    div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4 mb-4",
                        div { class: "bg-purple-900/30 border border-purple-700 rounded p-4",
                            h4 { class: "font-bold text-purple-300 mb-2", "OS Threads (Kernel)" }
                            p { class: "text-xs text-gray-300 mb-2",
                                "Created & scheduled by OS kernel. Each has own stack (1-8MB). Expensive (~10k cycles)."
                            }
                            p { class: "text-xs text-gray-400",
                                "Examples: std::thread, pthread, Rayon, spawn_blocking"
                            }
                        }
                        div { class: "bg-green-900/30 border border-green-700 rounded p-4",
                            h4 { class: "font-bold text-green-300 mb-2", "Green Threads" }
                            p { class: "text-xs text-gray-300 mb-2",
                                "Created by runtime, not OS. Very lightweight (~KB stack). Can have millions."
                            }
                            p { class: "text-xs text-gray-400",
                                "Examples: Go goroutines, Erlang processes"
                            }
                        }
                        div { class: "bg-blue-900/30 border border-blue-700 rounded p-4",
                            h4 { class: "font-bold text-blue-300 mb-2", "Async Tasks" }
                            p { class: "text-xs text-gray-300 mb-2",
                                "Not threads—"
                                span { class: "text-white font-semibold", "state machines" }
                                " that pause/resume. Multiplexed on few OS threads."
                            }
                            p { class: "text-xs text-gray-400",
                                "Examples: Tokio tasks, JS Promises, Python asyncio"
                            }
                        }
                    }

                    // Thread Roles
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Thread Roles (by Purpose)" }
                    div { class: "bg-gray-900 rounded p-4 mb-4",
                        p { class: "text-sm text-gray-300 mb-3",
                            "These roles can be filled by OS threads or green threads—the role is about what code they run:"
                        }
                        div { class: "grid grid-cols-1 md:grid-cols-2 gap-2 text-xs text-gray-300",
                            div {
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "Main thread" }
                                    " – entry point, coordinator or event loop"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "Worker threads" }
                                    " – execute tasks from a queue"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "I/O threads" }
                                    " – handle I/O polling/completion"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "Blocking threads" }
                                    " – isolated pool for sync/blocking work"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "Timer thread" }
                                    " – manages scheduled/delayed tasks"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "Signal handler" }
                                    " – catches OS signals (SIGTERM, etc.)"
                                }
                            }
                            div {
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "GUI/UI thread" }
                                    " – owns render loop, event processing"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "GC thread" }
                                    " – garbage collection (Java, Go, etc.)"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "Finalizer thread" }
                                    " – runs destructors/cleanup (JVM)"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "Watchdog thread" }
                                    " – monitors health, triggers recovery"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "Logger thread" }
                                    " – async log writing to avoid blocking"
                                }
                                p { class: "mb-1",
                                    span { class: "text-blue-300 font-semibold", "Daemon threads" }
                                    " – periodic tasks, housekeeping"
                                }
                            }
                        }
                        p { class: "text-xs text-gray-400 mt-3",
                            "Not every program has all of these. A simple CLI might just have main. A complex server might have several."
                        }
                    }

                    // The Two Types of Work
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Two Types of Work" }
                    div { class: "grid grid-cols-1 lg:grid-cols-2 gap-4 mb-4",
                        div { class: "bg-blue-900/30 border border-blue-700 rounded p-4",
                            h4 { class: "font-bold text-blue-300 mb-2", "I/O-Bound (Waiting)" }
                            p { class: "text-sm text-gray-300 mb-2",
                                "CPU sits idle while waiting for external resources."
                            }
                            ul { class: "text-sm text-gray-300 list-disc ml-4 space-y-1",
                                li { "HTTP requests" }
                                li { "Database queries" }
                                li { "File reads/writes" }
                                li { "Redis cache" }
                            }
                            p { class: "text-xs text-blue-300 mt-2", "→ Use Tokio (async)" }
                        }
                        div { class: "bg-green-900/30 border border-green-700 rounded p-4",
                            h4 { class: "font-bold text-green-300 mb-2", "CPU-Bound (Computing)" }
                            p { class: "text-sm text-gray-300 mb-2",
                                "CPU is actively working, no waiting."
                            }
                            ul { class: "text-sm text-gray-300 list-disc ml-4 space-y-1",
                                li { "Embedding generation" }
                                li { "Text chunking" }
                                li { "Vector similarity" }
                                li { "Reranking" }
                            }
                            p { class: "text-xs text-green-300 mt-2",
                                "→ Use Rayon or spawn_blocking"
                            }
                        }
                    }

                    // Tokio
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Tokio: Async Runtime" }
                    div { class: "bg-gray-900 rounded p-4 mb-4",
                        p { class: "text-sm text-gray-300 mb-2",
                            "Tokio handles thousands of concurrent I/O operations with few threads."
                        }
                        pre { class: "text-xs text-green-300 font-mono bg-gray-950 p-3 rounded mt-2",
                            "// Many requests, few threads
async fn handle_request() {{
    let data = db.query().await;  // Thread freed while waiting
    let cache = redis.get().await; // Thread freed while waiting
    // Tokio juggles thousands of these
}}"
                        }
                        p { class: "text-xs text-gray-400 mt-2",
                            "Your AG project: HTTP handlers, Redis cache, database queries"
                        }
                    }

                    // Rayon
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Rayon: Parallel CPU Work" }
                    div { class: "bg-gray-900 rounded p-4 mb-4",
                        p { class: "text-sm text-gray-300 mb-2",
                            "Rayon splits CPU work across all cores automatically."
                        }
                        pre { class: "text-xs text-green-300 font-mono bg-gray-950 p-3 rounded mt-2",
                            "use rayon::prelude::*;

// Process chunks in parallel across all CPU cores
let embeddings: Vec<_> = chunks
    .par_iter()  // Parallel iterator
    .map(|chunk| generate_embedding(chunk))
    .collect();"
                        }
                        p { class: "text-xs text-gray-400 mt-2",
                            "Your AG project: retriever.rs, batch.rs, product_quantization.rs"
                        }
                    }

                    // spawn_blocking
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "spawn_blocking: CPU Work in Async Context"
                    }
                    div { class: "bg-gray-900 rounded p-4 mb-4",
                        p { class: "text-sm text-gray-300 mb-2",
                            "When you need to do CPU work inside an async function, use spawn_blocking to avoid blocking the Tokio runtime."
                        }
                        pre { class: "text-xs text-green-300 font-mono bg-gray-950 p-3 rounded mt-2",
                            "// In embedder.rs
let embedding = task::spawn_blocking(move || {{
    runtime.embed_owned(text)  // CPU-heavy, runs on blocking thread
}}).await?;"
                        }
                        p { class: "text-xs text-yellow-300 mt-2",
                            "⚠️ Never do heavy CPU work directly in async fn - it blocks other requests!"
                        }
                    }

                    // When to Use What
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "When to Use What" }
                    table { class: "table table-sm w-full text-gray-300 mb-4",
                        thead {
                            tr {
                                th { class: "text-gray-200", "Task" }
                                th { class: "text-gray-200", "Tool" }
                                th { class: "text-gray-200", "Why" }
                            }
                        }
                        tbody {
                            tr {
                                td { "HTTP request" }
                                td { class: "text-blue-300", "tokio::spawn" }
                                td { "I/O wait" }
                            }
                            tr {
                                td { "Database query" }
                                td { class: "text-blue-300", "async/await" }
                                td { "I/O wait" }
                            }
                            tr {
                                td { "Batch embeddings" }
                                td { class: "text-green-300", "rayon par_iter" }
                                td { "CPU parallel" }
                            }
                            tr {
                                td { "Single embedding" }
                                td { class: "text-yellow-300", "spawn_blocking" }
                                td { "CPU in async" }
                            }
                            tr {
                                td { "Vector search" }
                                td { class: "text-green-300", "rayon" }
                                td { "CPU parallel" }
                            }
                            tr {
                                td { "File read" }
                                td { class: "text-blue-300", "tokio::fs" }
                                td { "I/O wait" }
                            }
                        }
                    }

                    // Your AG App
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "In your AG App" }
                    div { class: "bg-gray-700 rounded p-4 text-sm text-gray-200",
                        p { class: "mb-2",
                            "✅ Tokio 1.47 with full features - HTTP, Redis, async tasks"
                        }
                        p { class: "mb-2",
                            "✅ Rayon 1.10 - retriever.rs, batch.rs, product_quantization.rs"
                        }
                        p { class: "mb-2", "✅ spawn_blocking - embedder.rs for ONNX inference" }
                        p {
                            "Your RAG system correctly uses async for I/O and parallel threads for CPU work."
                        }
                    }

                    button {
                        class: "btn btn-primary btn-sm mt-6 w-full",
                        onclick: move |_| show_threads_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // Entities Production Modal
        if show_entities_production_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",

                div { class: "p-6 max-w-4xl mx-auto pb-20",

                    // Close button
                    div { class: "flex justify-between items-start mb-4",
                        h2 { class: "text-2xl font-bold text-white", "Entities Production" }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_entities_production_modal.set(false),
                            "×"
                        }
                    }

                    // Introduction
                    p { class: "text-lg text-gray-200 mb-4",
                        "Entities aren't \"generated\" automatically by a knowledge graph system—they are "
                        strong { "created from your data" }
                        ". But the way they are produced depends on the pipeline you build."
                    }

                    // How Entities Are Produced
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "How Entities Are Produced" }

                    // 1. From Source Data
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "1. Entities come from your source data"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Any concrete, real-world thing in your dataset becomes an entity."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Examples:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "A row in a database → entity" }
                        li { "A JSON object → entity" }
                        li { "A document → entity" }
                        li { "A user profile → entity" }
                        li { "A product → entity" }
                        li { "A location → entity" }
                    }
                    p { class: "text-sm text-gray-300 mt-3",
                        "If your data contains specific things, those become entity nodes."
                    }
                    div { class: "bg-gray-700 rounded p-3 my-3 font-mono text-xs text-green-300",
                        "Row: {{ id: 42, name: \"Paris\", population: 2.1M }}"
                        br {}
                        "→ (:City {{id: 42, name: \"Paris\", population: 2.1M}})"
                    }

                    // 2. Extracted from Text
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "2. Entities can also be extracted from text"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "If you run NLP or an ONNX model for NER (Named Entity Recognition), you can detect entities inside text."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Example sentence:" }
                    div { class: "bg-gray-700 rounded p-3 my-2 text-sm text-gray-200 italic",
                        "\"Microsoft acquired GitHub in 2018.\""
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-2", "NER model outputs:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li {
                            span { class: "text-blue-300", "Microsoft" }
                            " → Organization entity"
                        }
                        li {
                            span { class: "text-blue-300", "GitHub" }
                            " → Organization entity"
                        }
                        li {
                            span { class: "text-blue-300", "2018" }
                            " → Date entity"
                        }
                    }
                    p { class: "text-sm text-gray-300 mt-2", "These become nodes in Neo4j." }

                    // 3. Manual Schema
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "3. Entities can be created manually in your graph schema"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "When designing a knowledge graph, you often define:"
                    }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Entity types (Person, City, Product, Event)" }
                        li { "Their properties" }
                        li { "Their relationships" }
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "Then you populate them from your data sources."
                    }

                    // 4. Embedding-based Clustering
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "4. Entities can be produced by embedding-based clustering"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "This is where ONNX + Neo4j becomes powerful."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Workflow:" }
                    ol { class: "space-y-1 text-sm text-gray-300 ml-4 list-decimal",
                        li { "Generate embeddings for documents, sentences, or items" }
                        li { "Cluster similar embeddings" }
                        li { "Each cluster becomes a new entity or category" }
                        li { "Nodes inside the cluster become entity instances" }
                    }
                    p { class: "text-sm text-gray-300 mt-3 mb-2", "This is common in:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Product catalogs" }
                        li { "Scientific literature graphs" }
                        li { "Customer segmentation" }
                        li { "Semantic search systems" }
                    }

                    // 5. Inferred from Graph Structure
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "5. Entities can be inferred from graph structure"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Sometimes you don't explicitly create an entity—it emerges from relationships."
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Example: If many documents reference \"Graph Neural Networks\", you might create a new entity node:"
                    }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "(:Concept {{name: \"Graph Neural Networks\"}})"
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "But if you detect a specific paper titled \"Graph Neural Networks in Practice\", that becomes an entity."
                    }

                    // Summary Table
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "Summary Table" }
                    table { class: "table table-sm w-full text-gray-300 mb-4",
                        thead {
                            tr {
                                th { class: "text-gray-200", "Source" }
                                th { class: "text-gray-200", "How Entities Are Produced" }
                            }
                        }
                        tbody {
                            tr {
                                td { "Structured data" }
                                td { "Each row/object becomes an entity node" }
                            }
                            tr {
                                td { "Text (NER)" }
                                td { "ONNX/NLP models extract named entities" }
                            }
                            tr {
                                td { "Manual modeling" }
                                td { "You define entity types and create nodes" }
                            }
                            tr {
                                td { "Embeddings + clustering" }
                                td { "Semantic groups become entity nodes" }
                            }
                            tr {
                                td { "Graph inference" }
                                td { "Patterns in relationships reveal new entities" }
                            }
                        }
                    }

                    // How This Connects to Embeddings and Neo4j
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3",
                        "How This Connects to Embeddings and Neo4j"
                    }
                    ul { class: "space-y-2 text-sm text-gray-300 ml-4 list-disc",
                        li { "ONNX generates embeddings for text, images, or objects" }
                        li { "You store those embeddings on entity nodes in Neo4j" }
                        li { "Neo4j uses vector search + graph structure to connect entities" }
                        li { "Entities become the \"specific things\" your graph reasons about" }
                    }

                    div { class: "bg-gray-700 rounded p-4 my-4 border-l-4 border-blue-500",
                        p { class: "text-gray-200 mb-2",
                            strong { "Concepts" }
                            " = abstract categories"
                        }
                        p { class: "text-gray-200",
                            strong { "Entities" }
                            " = concrete instances"
                        }
                    }
                
                }
            }
        }
    }
}
p { class: "text-sm text-gray-300 mt-4 mb-2", "Embeddings help you:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "detect entities" }
                        li { "classify entities" }
                        li { "link entities" }
                        li { "cluster entities" }
                        li { "search entities" }
                    }

                    // How Concepts and Entities Matter for Embeddings, ONNX, and Neo4j
                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "How Concepts and Entities Matter for Embeddings, ONNX, and Neo4j" }
                    p { class: "text-sm text-gray-300 mb-4",
                        "A knowledge graph uses two kinds of nodes:"
                    }

                    // Concepts
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2", "Concepts" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Abstract categories or types. They describe what kind of thing something is."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Examples:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Animal" }
                        li { "City" }
                        li { "Programming language" }
                        li { "Company" }
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "Concepts are general and not tied to a single instance."
                    }

                    // Entities
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2", "Entities" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Concrete, specific things in the real world. They describe which particular thing we mean."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Examples:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Fido (a specific dog)" }
                        li { "Paris" }
                        li { "Python 3.12" }
                        li { "Microsoft" }
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "Entities are unique instances of concepts."
                    }

                    // How This Relates to Embeddings
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2", "How This Relates to Embeddings" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Embeddings give you a vector representation:"
                    }
                    div { class: "bg-gray-700 rounded p-3 my-2 text-center",
                        code { class: "text-lg text-blue-300", "v ∈ ℝⁿ" }
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Meaning:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "∈ means \"is an element of\"" }
                        li { "The embedding vector (v) belongs to an n-dimensional real vector space" }
                        li { "Every concept and entity can have its own embedding" }
                    }
                    p { class: "text-sm text-gray-300 mt-3",
                        "This is where things get interesting."
                    }

                    // Concepts get concept embeddings
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2", "Concepts get concept embeddings" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "These capture the general meaning of a category."
                    }
                    p { class: "text-sm text-gray-300",
                        "Example: The embedding for \"City\" encodes the idea of cities in general."
                    }

                    // Entities get entity embeddings
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2", "Entities get entity embeddings" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "These capture the specific meaning of a particular instance."
                    }
                    p { class: "text-sm text-gray-300",
                        "Example: The embedding for \"Paris\" encodes the specific city, not the general idea of cities."
                    }

                    p { class: "text-sm text-gray-300 mt-4 mb-2",
                        "Because embeddings live in the same vector space, you can compare:"
                    }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Entity ↔ Entity" }
                        li { "Entity ↔ Concept" }
                        li { "Concept ↔ Concept" }
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "This lets you do semantic reasoning on top of your graph."
                    }

                    // How ONNX Fits In
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2", "How ONNX Fits In" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "ONNX is the runtime that generates embeddings."
                    }
                    p { class: "text-sm text-gray-300 mb-1", "You feed it:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "text" }
                        li { "sentences" }
                        li { "documents" }
                        li { "images" }
                        li { "node labels" }
                        li { "descriptions" }
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-1", "It outputs:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "a vector (v ∈ ℝⁿ)" }
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-1", "You can generate embeddings for:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Concepts (\"City\", \"Programming language\")" }
                        li { "Entities (\"Paris\", \"Python 3.12\")" }
                        li { "Relationships (\"located in\", \"is a\")" }
                        li { "Whole subgraphs (if you encode them)" }
                    }

                    // How Neo4j Fits In
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2", "How Neo4j Fits In" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Neo4j is the graph database that stores and queries embeddings."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "You attach embeddings to nodes:" }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "(:Concept {{name: \"City\", embedding: [...]}})"
                        br {}
                        "(:Entity  {{name: \"Paris\", embedding: [...]}})"
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-1", "Neo4j can then:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Perform vector similarity search" }
                        li { "Combine vector similarity with graph structure" }
                        li { "Infer relationships" }
                        li { "Cluster similar nodes" }
                        li { "Support RAG pipelines" }
                        li { "Build hybrid symbolic + semantic reasoning systems" }
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "This is where the power emerges."
                    }

                    // Putting It All Together
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2", "Putting It All Together" }
                    p { class: "text-sm text-gray-300 mb-2", "Here's the full pipeline:" }
                    div { class: "bg-gray-700 rounded p-4 my-2 font-mono text-xs text-blue-300 whitespace-pre",
                        "          ONNX Model\n"
                        "    (Embedding Generator)\n"
                        "              │\n"
                        "              ▼\n"
                        "  v ∈ ℝⁿ (embedding vector)\n"
                        "              │\n"
                        "              ▼\n"
                        "        Neo4j Graph\n"
                        " (Concepts + Entities + Vectors)\n"
                        "              │\n"
                        "              ▼\n"
                        "Semantic search + graph reasoning"
                    }

                    // Example
                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2", "Example" }
                    p { class: "text-sm text-gray-300 mb-2", "Concept:" }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "(:Concept {{name: \"City\", embedding: [...]}})"
                    }
                    p { class: "text-sm text-gray-300 mt-3 mb-2", "Entity:" }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "(:Entity {{name: \"Paris\", embedding: [...]}})"
                    }
                    p { class: "text-sm text-gray-300 mt-3 mb-2", "Query:" }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "MATCH (e:Entity)"
                        br {}
                        "RETURN e, gds.similarity.cosine(e.embedding, $queryEmbedding) AS score"
                        br {}
                        "ORDER BY score DESC"
                        br {}
                        "LIMIT 5"
                    }
                    p { class: "text-sm text-gray-300 mt-3", "You can now:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Find cities similar to Paris" }
                        li { "Find entities similar to a concept" }
                        li { "Build hybrid reasoning systems that mix symbolic edges with semantic vectors" }
                    }

                    button {
                        class: "btn btn-primary btn-sm mt-8 w-full",
                        onclick: move |_| show_entities_production_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }
    }  
}      