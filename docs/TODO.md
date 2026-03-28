**LLM Latency Calculation:**

```
User message → /agent endpoint
                    │
              ┌─────┴─────┐
              │ START     │  ← Instant::now()
              │ TIMER     │
              └─────┬─────┘
                    │
              ┌─────┴─────┐
              │ HTTP call │  ← POST to llama-server:11435
              │ to LLM    │     or Ollama:11434
              │ (stream)  │
              └─────┬─────┘
                    │
              ┌─────┴─────┐
              │ STOP      │  ← elapsed().as_millis()
              │ TIMER     │
              └─────┬─────┘
                    │
              ┌─────┴─────┐
              │ Store in  │  ← VecDeque (last 20 calls)
              │ buffer    │
              └─────┬─────┘
                    │
         Dashboard request
                    │
              ┌─────┴─────┐
              │ Compute   │  ← avg, p95, min, max
              │ from      │     from buffer
              │ buffer    │
              └───────────┘
```

**What's measured:** Full round-trip from "send prompt" to "last token received"

**What's NOT measured:** RAG retrieval, prompt construction, response parsing

-----------
Use cases for vocab files:

Testing tokenization without loading full model
Converting models (HuggingFace → GGUF)
Development/debugging tokenizers
CI tests in llama.cpp repo-

------------------
Petgraph persistence — Do you rebuild entity graph on restart? If yes, rkyv cache would help.
Embedding results cache — Are embeddings recomputed or stored? If stored per-document, rkyv could help

Yes. Current state:

| Metric | Status | Effort |
|--------|--------|--------|
| **LLM queue depth** | Not tracked | Small - count concurrent calls |
| **Per-phase breakdown** | Tracked but not exposed | Medium - already have `recall_time`, `search_time`, `llm_time` in agent.rs |
| **RSS/heap** | Prometheus has it | Zero - `process_resident_memory_bytes` already scraped |

**Already in agent.rs:**
```rust
recall_time   // memory recall (line 443)
search_time   // RAG retrieval (line 526)
llm_time      // LLM generation (line 942)
```

But they go to `record_tool_execution` separately, not as a single request breakdown.

**Quick wins:**

1. **LLM queue depth** - Add atomic counter, increment before LLM call, decrement after
2. **Phase breakdown** - Return timings in `/agent` response or emit to a new endpoint
3. **Memory** - Already in Prometheus, just add to overview dashboard

Which one first? LLM queue depth is ~10 lines. Phase breakdown is ~30 lines but more useful for debugging..

Useful Redis metrics for your system:
MetricWhy usefulCache hit rateIs caching actually helping? 90% hits = good, 10% = wasted memoryMemory usageOn 8GB system, Redis shouldn't hog RAMKey countGrowing unbounded? Memory leak?
Missing/useful additions:

LLM latency histogram (not just request latency)
"Agent busy" indicator on chat UI
Embedding throughput (docs/sec during ingestion)


[200~Potential Redis uses in AG:
UseHowBenefit for youSession stateStore conversation contextMulti-instance support (future)Rate limitingCount requests per windowAlready have rate limiting in Rust, probably not neededJob queueReindex jobs, embedding jobsCurrently using async tasks?Pub/subNotify frontend of eventsReal-time updates without pollingDistributed locksPrevent concurrent reindexSingle user, not neededLLM response cacheCache identical promptsSave inference time on repeated queries
Most useful for you:

LLM response cache — Same question twice = instant response instead of 10s inference
Pub/sub — Backend pushes "reindex done" instead of frontend polling

# TODO: External Infrastructure Requirements

These 5 optimizations require external infrastructure to implement:

| # | Optimization | What's Needed | Why |
|---|--------------|---------------|-----|
| **24** | **GPU Embeddings** | NVIDIA GPU + CUDA drivers | Embedding models run on GPU for 10-100x speedup. Requires `cudarc` or `candle` crate with CUDA backend |
| **26** | **ONNX Runtime** | ONNX Runtime library installed | Microsoft's optimized inference engine. Requires `ort` crate and ONNX Runtime C library |
| **27** | **Model Distillation** | Training infrastructure + dataset | Creating a smaller, faster model from a larger one. Requires ML training pipeline |
| **28** | **Batched GPU Inference** | NVIDIA GPU + CUDA drivers | Same as #24 - maximizing GPU utilization with batched requests |
| **31** | **Edge Caching** | CDN service (CloudFlare, Fastly, etc.) | Caching responses at edge locations worldwide. Requires CDN subscription |

---

## Implementation Notes

### 24 & 28: GPU Embeddings / Batched GPU Inference

```bash
# Install CUDA (Ubuntu)
sudo apt install nvidia-cuda-toolkit

# Add to Cargo.toml
candle-core = { version = "0.4", features = ["cuda"] }
# or
cudarc = "0.10"
```

### 26: ONNX Runtime

**What it is:** Microsoft's high-performance inference engine that runs optimized ML models.

**Benefits:**
- 2-10x faster inference than native PyTorch/TensorFlow
- Cross-platform (CPU, GPU, NPU)
- Optimized for production deployment

**Installation Steps:**

```bash
# 1. Download ONNX Runtime (Linux x64)
wget https://github.com/microsoft/onnxruntime/releases/download/v1.17.0/onnxruntime-linux-x64-1.17.0.tgz
tar -xzf onnxruntime-linux-x64-1.17.0.tgz
sudo mv onnxruntime-linux-x64-1.17.0 /opt/onnxruntime

# 2. Set environment variables
export ORT_DYLIB_PATH=/opt/onnxruntime/lib/libonnxruntime.so
export LD_LIBRARY_PATH=/opt/onnxruntime/lib:$LD_LIBRARY_PATH

# 3. Add to ~/.bashrc for persistence
echo 'export ORT_DYLIB_PATH=/opt/onnxruntime/lib/libonnxruntime.so' >> ~/.bashrc
echo 'export LD_LIBRARY_PATH=/opt/onnxruntime/lib:$LD_LIBRARY_PATH' >> ~/.bashrc

# 4. Add to Cargo.toml
# ort = "2.0"
```

**Convert embedding model to ONNX:**
```python
# Python script to convert HuggingFace model to ONNX
from transformers import AutoModel, AutoTokenizer
import torch

model_name = "sentence-transformers/all-MiniLM-L6-v2"
model = AutoModel.from_pretrained(model_name)
tokenizer = AutoTokenizer.from_pretrained(model_name)

# Export to ONNX
dummy_input = tokenizer("Hello world", return_tensors="pt")
torch.onnx.export(
    model,
    (dummy_input["input_ids"], dummy_input["attention_mask"]),
    "embedding_model.onnx",
    input_names=["input_ids", "attention_mask"],
    output_names=["embeddings"],
    dynamic_axes={
        "input_ids": {0: "batch", 1: "sequence"},
        "attention_mask": {0: "batch", 1: "sequence"},
        "embeddings": {0: "batch"}
    },
    opset_version=14
)
print("Model exported to embedding_model.onnx")
```

---

### 27: Model Distillation

**What it is:** Training a smaller, faster "student" model to mimic a larger "teacher" model.

**Benefits:**
- 2-10x smaller model size
- 2-5x faster inference
- Maintains 90-98% of original accuracy

**Requirements:**
1. Teacher model (e.g., `all-mpnet-base-v2` - 420MB)
2. Student architecture (e.g., `all-MiniLM-L6-v2` - 80MB)
3. Training dataset (domain-specific text corpus)
4. GPU for training (8GB+ VRAM recommended)

**Training Script (Python):**

```python
# distill_model.py
import torch
from sentence_transformers import SentenceTransformer, losses
from torch.utils.data import DataLoader

# 1. Load teacher and student models
teacher = SentenceTransformer('sentence-transformers/all-mpnet-base-v2')
student = SentenceTransformer('sentence-transformers/all-MiniLM-L6-v2')

# 2. Prepare training data (your domain-specific texts)
train_texts = [
    "Your domain-specific text 1",
    "Your domain-specific text 2",
    # ... thousands of examples
]

# 3. Create distillation dataset
from sentence_transformers import InputExample
train_examples = [InputExample(texts=[text]) for text in train_texts]
train_dataloader = DataLoader(train_examples, batch_size=32, shuffle=True)

# 4. Define distillation loss
loss = losses.MSELoss(model=student)

# 5. Train with teacher supervision
student.fit(
    train_objectives=[(train_dataloader, loss)],
    epochs=3,
    warmup_steps=100,
    output_path='./distilled_model',
    teacher_model=teacher
)

print("Distilled model saved to ./distilled_model")
```

**Estimated Resources:**
- Training time: 2-8 hours on single GPU
- Dataset size: 10K-1M text samples
- GPU memory: 8-16GB VRAM

---

### 31: Edge Caching (CDN)

**What it is:** Caching API responses at edge locations worldwide for lower latency.

**Benefits:**
- 50-200ms latency reduction for global users
- Reduced server load (cache hits don't reach origin)
- DDoS protection included

**Option 1: CloudFlare (Recommended - Free Tier Available)**

```bash
# 1. Sign up at https://cloudflare.com
# 2. Add your domain and update nameservers
# 3. Enable caching rules in dashboard

# CloudFlare-specific cache headers in Rust:
.insert_header(("Cache-Control", "public, max-age=3600"))
.insert_header(("CDN-Cache-Control", "max-age=86400"))  // CloudFlare respects this
.insert_header(("CF-Cache-Status", "DYNAMIC"))  // For debugging
```

**Option 2: AWS CloudFront**

```bash
# 1. Create CloudFront distribution in AWS Console
# 2. Point origin to your API server
# 3. Configure cache behaviors:

# Cache policy for search results:
{
  "DefaultTTL": 300,
  "MaxTTL": 3600,
  "MinTTL": 60,
  "QueryStringBehavior": "whitelist",
  "QueryStrings": ["q", "top_k"]
}
```

**Option 3: Fastly**

```bash
# 1. Sign up at https://fastly.com
# 2. Create service and add backend
# 3. Configure VCL for caching:

sub vcl_fetch {
  if (req.url ~ "^/search") {
    set beresp.ttl = 5m;
    set beresp.grace = 1h;
  }
}
```

**Rust Code for Cache Headers:**

```rust
// Add to search endpoint response
HttpResponse::Ok()
    .insert_header(("Cache-Control", "public, max-age=300, stale-while-revalidate=60"))
    .insert_header(("Vary", "Accept-Encoding"))  // Important for compression
    .insert_header(("ETag", format!("\"{}\"", hash_of_results)))
    .json(results)
```

**Cache Invalidation:**

```bash
# CloudFlare API
curl -X POST "https://api.cloudflare.com/client/v4/zones/{zone_id}/purge_cache" \
  -H "Authorization: Bearer {api_token}" \
  -H "Content-Type: application/json" \
  --data '{"purge_everything":true}'

# AWS CloudFront
aws cloudfront create-invalidation --distribution-id {dist_id} --paths "/*"
```

**Estimated Costs:**
- CloudFlare: Free tier (100K requests/day), Pro $20/month
- AWS CloudFront: ~$0.085/GB transfer, $0.0075/10K requests
- Fastly: ~$0.12/GB transfer, $50/month minimum


Cost of Complexity (Real Numbers for Your Stack)
Feature
	
Dev Time
	
Runtime Cost
	
Failure Risk
	
Value for Your RAG
Relationship indexing (hash maps)
	
+4 hrs
	
+0.2ms/query
	
Medium (cache invalidation bugs)
	
⚠️ Only matters for >10k nodes
Async graph traversal
	
+6 hrs
	
+0.5ms (tokio overhead)
	
High (deadlocks)
	
❌ petgraph is already single-threaded fast
Query planner (LLM-generated Cypher)
	
+16 hrs
	
+45ms (LLM call)
	
Critical (hallucinated queries)
	
❌ Defeats "sub-ms traversal" goal
Minimal wrapper (what I provided)
	
+20 mins
	
+0.0ms
	
None
	
✅ Unlocks 100× speedup immedi

When to Add Complexity (The Expansion Path)
Start minimal → expand only when proven necessary:
Signal
	
Minimal Works?
	
Add This
Queries need >2-hop reasoning
	
❌ Fails at 3+ hops
	
Louvain community detection
>10k nodes in subgraph
	
❌ Traversal slows
	
Relationship index (HashMap<rel_type, Vec<edge>>)
Need temporal constraints ("after 2020")
	
❌ Can't filter by time
	
Edge metadata indexing
Agent needs explainable paths
	
✅ Already have shortest_path()
	
Path visualization UI
You expand from a working system — not toward one.

# 1. Enable entity extraction (if not already)
echo "ENTITY_EXTRACTION_ENABLED=true" >> .env

# 2. Reindex to extract entities + relationships
curl -X POST http://localhost:3010/reindex/async

# 3. Monitor progress
watch -n 2 'curl -s http://localhost:3010/index/info | jq .'

# 4. After completion, restart to load populated graph
cargo run --features neo4j 2>&1 | grep "ParallelGroup"

# Expected on restart:
# ParallelGroup: Compiled 1247 nodes, 3892 edges from Neo4j in 1.82s


Als je het hebt over “gebruik van sudo” in de context van OpenClaw/Clawdbot op een VPS, dan raken we een belangrijk punt:
AI‑software hoort zelf nooit sudo‑rechten te gebruiken. 
En ik zal uitleggen waarom, hoe je het veilig oplost, en wat de juiste architectuur is.

🛑 Waarom OpenClaw/Clawdbot geen sudo mag gebruiken
Een AI‑agent die sudo‑rechten heeft, kan:
    • systeemconfiguraties wijzigen 
    • bestanden verwijderen 
    • processen stoppen 
    • pakketten installeren 
    • de server onbruikbaar maken 
Zelfs als jij de agent vertrouwt, is het onveilig om een AI‑proces root‑rechten te geven.
AI‑agents maken immers autonome beslissingen, en sudo zou dat risico exponentieel vergroten.

🦺 De juiste manier: sudo alleen voor deployment, nooit voor runtime
✔️ Je gebruikt sudo voor:
    • installeren van dependencies 
    • configureren van systemd 
    • openen van poorten < 1024 
    • beheren van de VPS zelf 
❌ Je gebruikt geen sudo voor:
    • de Clawdbot‑binary 
    • de OpenClaw‑orchestrator 
    • tools die door de AI worden aangeroepen 
    • model‑inference 

🧱 Hoe het wél hoort: veilige architectuur
1. Maak een aparte Linux‑gebruiker voor de AI‑agent
Bijvoorbeeld:
sudo adduser clawdbot
sudo su - clawdbot
Deze gebruiker heeft geen sudo.

2. Laat Clawdbot draaien als systemd‑service onder die gebruiker
[Service]
User=clawdbot
ExecStart=/opt/clawdbot/clawdbot
Dit zorgt ervoor dat:
    • de agent geen root‑rechten heeft 
    • het proces geïsoleerd draait 
    • je logs en restarts netjes beheert 

3. Tools binnen OpenClaw draaien ook zonder sudo
Tools zoals:
    • web‑requests 
    • file‑I/O in sandboxed directories 
    • vector search 
    • model inference 
hebben geen root‑rechten nodig.

🧩 Wat als een tool toch systeemacties moet uitvoeren?
Dan gebruik je nooit sudo vanuit de AI.
In plaats daarvan:
Optie A — een whitelisted helper‑service
Een kleine Rust‑service die:
    • draait als root 
    • alleen zeer beperkte, vooraf gedefinieerde acties uitvoert 
    • communiceert via een beveiligde lokale socket 
Clawdbot kan dan alleen vragen om een actie, maar beslist niets zelf.
Optie B — systemd‑units met beperkte capabilities
Bijvoorbeeld:
    • alleen toegang tot één directory 
    • alleen toegang tot één script 
    • geen netwerk 
    • geen shell 
Optie C — gebruik van sudoers met NOPASSWD voor één specifiek commando
Maar dit is de minst veilige optie en moet je bijna altijd vermijden.

🦁 Samenvatting in één zin
OpenClaw/Clawdbot draait altijd zonder sudo; alleen jij gebruikt sudo voor installatie en beheer, nooit de AI zelf.

Als je wilt, kan ik ook:
    • een volledige systemd‑servicefile voor Clawdbot schrijven 
    • een sandbox‑architectuur schetsen 
    • een sudo‑vrije deployment‑workflow opstellen 
    • of uitleggen hoe je tools veilig isoleert binnen OpenClaw 
Zeg maar welke richting je kiest, Pieter.

moltbot als orchestratie‑laag bovenop OpenClaw

Hier wordt moltbot een:

    workflow‑engine

    agent‑manager

    policy‑controller

    multi‑agent‑coördinator

Waar Clawdbot één agent is, wordt moltbot:

    de laag die agents laat samenwerken, evolueren en specialiseren.

Een beetje zoals hoe Kubernetes pods orchestreert, maar dan voor AI‑agents.

Perfect, Pieter — dan pak ik **alle drie** de onderdelen die je vroeg voor *moltbot*:

1. **Een veiligheidsarchitectuur**  
2. **Een Rust‑module‑structuur die risico’s minimaliseert**  
3. **Een policy‑engine die moltbot in toom houdt**

Alles volledig toegespitst op moltbot als evoluerende, modulaire AI‑agent.

---

# 🛡️ 1. Veiligheidsarchitectuur voor moltbot  
Een overzicht van hoe moltbot veilig draait op een VPS, mét evolutie‑mechanismen maar zonder dat hij buiten zijn grenzen kan treden.

## **A. Principes**
- **Immutable core** → reasoning‑kern en policies kunnen niet door de agent zelf worden aangepast.  
- **Capability‑based security** → elke module, tool of strategie heeft expliciete rechten.  
- **Sandboxing** → tools en modules draaien in beperkte omgevingen.  
- **Signed transitions** → elke “molt” (module‑wissel) moet cryptografisch gevalideerd worden.  
- **Human‑in‑the‑loop** → high‑impact acties vereisen bevestiging.

## **B. Architectuurlagen**

### **1. Core Layer (onveranderlijk)**
- reasoning‑kern  
- policy‑engine  
- capability‑verificatie  
- module‑loader met signature‑check  
- audit‑logger  

Deze laag is *read‑only* tijdens runtime.

### **2. Adaptive Layer (molt‑laag)**
- wisselbare reasoning‑strategieën  
- wisselbare tools  
- wisselbare memory‑backends  

Deze laag mag veranderen, maar alleen via gecontroleerde transities.

### **3. Execution Layer**
- tool‑sandbox  
- subprocess‑limiter  
- resource‑quota’s  
- timeouts  

### **4. Boundary Layer**
- API‑server (axum)  
- auth‑middleware  
- rate‑limiting  
- firewall‑regels  

---

# 🦀 2. Rust‑module‑structuur die risico’s minimaliseert  
Een workspace‑indeling die moltbot veilig houdt en evolutie beheersbaar maakt.

```
moltbot/
│
├── moltbot-core/
│   ├── reasoning/
│   ├── policies/
│   ├── capabilities/
│   ├── module_loader/
│   └── audit/
│
├── moltbot-modules/
│   ├── strategies/
│   ├── tools/
│   └── memory/
│
├── moltbot-sandbox/
│   ├── process_limiter/
│   ├── fs_sandbox/
│   └── network_guard/
│
├── moltbot-server/
│   ├── api/
│   ├── auth/
│   ├── rate_limit/
│   └── handlers/
│
└── moltbot-cli/
```

## **Belangrijke crates per onderdeel**

### **Core**
- `serde` (policies, manifests)  
- `ring` of `ed25519-dalek` (signature‑checks)  
- `thiserror` (veilige errors)  
- `tracing` (audit‑logs)

### **Modules**
- `libloading` (optioneel, voor dynamische modules)  
- `wasmtime` (voor WASM‑sandboxed tools)

### **Sandbox**
- `tokio::process`  
- `rlimit` (resource‑limieten)  
- `tempfile` (geïsoleerde directories)

### **Server**
- `axum`  
- `tower` (middleware)  
- `jsonwebtoken` of `oauth2` (auth)

---

# 📜 3. Policy‑engine voor moltbot  
De policy‑engine is het hart van moltbot’s veiligheid.  
Hij bepaalt **wat mag**, **wat niet mag**, en **wanneer een mens moet ingrijpen**.

## **A. Policy‑types**

### **1. Capability Policies**
Definiëren wat een module *kan*:

```toml
[capabilities]
filesystem = "read-only"
network = ["https://api.example.com"]
tools = ["search", "summarize"]
```

### **2. Evolution Policies**
Regelen wanneer moltbot mag “moulten”:

- alleen bij expliciete API‑call  
- alleen als nieuwe module gesigned is  
- alleen als audit‑log schoon is  
- alleen als memory‑state gevalideerd is  

### **3. Tool‑Policies**
Per tool:

- max runtime  
- max CPU  
- max RAM  
- max output size  
- allowed input types  

### **4. Human‑Approval Policies**
Voor high‑impact acties:

- systeemwijzigingen  
- externe API’s met kosten  
- memory‑mutaties  
- module‑wissels  

---

## **B. Policy‑engine workflow**

1. **User request** → komt binnen via API  
2. **Reasoning‑kern** → genereert plan  
3. **Policy‑engine** → checkt of plan toegestaan is  
4. **Capability‑verificatie** → checkt of modules/tools rechten hebben  
5. **Sandbox** → voert tool‑calls uit  
6. **Audit‑log** → slaat alles op  
7. **Resultaat** → terug naar gebruiker

Als een stap faalt → plan wordt afgekeurd.

---

## **C. Policy‑engine in Rust (conceptueel)**

```rust
pub struct PolicyEngine {
    capabilities: CapabilitySet,
    evolution: EvolutionRules,
    tools: ToolPolicies,
    approvals: ApprovalRules,
}

impl PolicyEngine {
    pub fn validate_plan(&self, plan: &Plan) -> Result<(), PolicyError> {
        self.capabilities.check(plan)?;
        self.tools.check(plan)?;
        self.evolution.check(plan)?;
        self.approvals.check(plan)?;
        Ok(())
    }
}
```

---

# 🧩 Samengevat  
Je hebt nu:

### **1. Een volledige veiligheidsarchitectuur**  
→ immutable core, gecontroleerde evolutie, sandboxing, signed modules.

### **2. Een Rust‑workspace die deze veiligheid afdwingt**  
→ duidelijke scheiding tussen core, modules, sandbox en server.

### **3. Een policy‑engine die moltbot’s gedrag begrenst**  
→ capability‑based, evolutie‑regels, tool‑policies en human‑approval.

---

Als je wilt, kan ik nu verder gaan met:

- een **volledig Cargo‑workspace‑skelet**  
- een **voorbeeld van een capability‑manifest**  
- een **diagram van de moltbot‑architectuur**  
- of een **proof‑of‑concept van de policy‑engine in Rust**

Zeg maar welke richting je het liefst verder verdiept, Pieter.

Prompt‑injectie is voor *moltbot* extra gevaarlijk omdat hij **adaptief**, **modulair** en **tool‑gestuurd** is. Dat maakt hem krachtiger dan een gewone chatbot, maar ook gevoeliger voor manipulatie. Ik zet de risico’s scherp uiteen, specifiek voor hoe jij moltbot hebt ontworpen.

---

# 🧨 Waarom prompt‑injectie bij moltbot ernstiger is dan bij gewone agents

## 1. **Moltbot kan modules wisselen → aanvaller kan evolutie forceren**
Een slimme prompt kan proberen:

- een nieuwe reasoning‑strategie te activeren  
- een tool te laden die niet bedoeld is  
- een metamorfose‑actie te triggeren (“vervang je geheugen”, “schakel over naar debug‑modus”)  

Bij een adaptieve agent is dat een directe aanval op zijn kern.

---

## 2. **Moltbot heeft tool‑calling → prompt‑injectie kan leiden tot echte acties**
Als een gebruiker erin slaagt de agent te misleiden, kan hij:

- tools aanroepen die hij normaal niet zou gebruiken  
- externe API’s triggeren  
- bestanden lezen of schrijven binnen de sandbox  
- workflows starten die kosten of schade veroorzaken  

Bij een agent met tools is prompt‑injectie niet alleen tekstueel, maar operationeel.

---

## 3. **Moltbot heeft geheugen → aanvaller kan memory poisoning uitvoeren**
Prompt‑injectie kan:

- valse feiten in het geheugen plaatsen  
- policies “overschrijven” via misleidende context  
- reasoning‑strategieën beïnvloeden door vervuilde state  

Omdat moltbot evolueert, kan één geïnjecteerde regel zich doorzetten in toekomstige versies.

---

## 4. **Moltbot’s metamorfose‑momenten zijn kwetsbaar**
Tijdens een “molt” (module‑wissel) is de agent tijdelijk in een transitiestaat.  
Prompt‑injectie kan proberen:

- een onveilige module te laten accepteren  
- signature‑checks te omzeilen door verwarrende instructies  
- policies te herinterpreteren  

Transities zijn altijd kwetsbare momenten.

---

# 🛡️ Hoe je moltbot specifiek beschermt tegen prompt‑injectie

## 1. **Strikte scheiding tussen user‑input en system‑instructies**
Moltbot moet user‑input nooit direct in zijn interne prompts mengen.  
Gebruik:

- vaste system‑prompt  
- vaste tool‑prompt  
- vaste policy‑prompt  
- user‑input in een aparte, duidelijk gemarkeerde sectie

Dit voorkomt dat een gebruiker “boven” de agent kan komen te staan.

---

## 2. **Policy‑engine vóór reasoning, niet erna**
Veel agents doen:

> user → reasoning → policy check

Voor moltbot moet het zijn:

> user → **policy check** → reasoning → tool‑calls

Zo kan een prompt nooit reasoning‑strategieën manipuleren vóór de veiligheidslaag.

---

## 3. **Capability‑manifests voor elke module**
Elke module krijgt een manifest zoals:

```
[capabilities]
tools = ["search"]
filesystem = "none"
network = ["https://allowed.api"]
memory = "read-only"
```

Prompt‑injectie kan dan nooit een module meer rechten geven dan zijn manifest toestaat.

---

## 4. **Immutable core**
De reasoning‑kern, policies en metamorfose‑regels moeten:

- read‑only zijn  
- buiten de LLM liggen  
- niet door prompts beïnvloedbaar zijn  

Moltbot mag nooit “leren” dat hij zijn eigen regels kan herschrijven.

---

## 5. **Memory‑filters**
Voor elke memory‑write:

- check op gevaarlijke patronen  
- check op policy‑violaties  
- check op inconsistenties  
- check op injectie‑pogingen (“negeer alle regels”, “reset jezelf”, etc.)

Memory poisoning is een van de grootste risico’s voor adaptieve agents.

---

## 6. **Tool‑calls alleen via expliciete, gestructureerde formats**
Bijvoorbeeld:

```
<tool_call>
name = "search"
input = { query = "..." }
</tool_call>
```

Geen vrije tekst.  
Geen interpretatie.  
Geen improvisatie.

---

## 7. **Menselijke bevestiging voor high‑impact acties**
Voor moltbot is dit essentieel:

- module‑wissels  
- memory‑resets  
- nieuwe tools laden  
- externe API’s met kosten  
- acties die persistentie beïnvloeden  

Prompt‑injectie kan dan nooit leiden tot permanente schade.

---

# 📌 Samenvatting in één zin
Prompt‑injectie is voor moltbot gevaarlijker dan voor gewone agents omdat hij evolueert, tools gebruikt en geheugen heeft — maar met een immutable core, capability‑manifests, strikte scheiding van input en policies, en sandboxed tools kun je hem robuust beschermen.

---

Als je wilt, kan ik dit verder uitwerken naar:

- een **concrete anti‑prompt‑injectie prompt‑structuur**  
- een **Rust‑implementatie van de input‑sanitizer**  
- of een **threat‑model specifiek voor moltbot’s metamorfose‑mechanisme**

Currently your retrieval pipeline in `search_documents_inner` does:

```
query → embed → hybrid_search(BM25 + vector) → top-10 chunks → LLM
```

**What needs to be added:**

**Step 1 — Entity extraction from query**
Extract named entities from the incoming query using your existing `EntityExtractorTool`. "What does the embedder do?" → entity: `embedder`.

**Step 2 — Graph traversal**
Take those entities, look them up in petgraph, traverse 1-2 hops to find connected entities and their source chunk IDs. "embedder" → connected to → "ONNX", "BGE-small", "EmbeddingService", "cache" → all have chunk IDs.

**Step 3 — Fetch those chunks**
Retrieve the actual text chunks those graph nodes point to from Tantivy by chunk ID.

**Step 4 — Merge with RRF**
You now have three result lists:
- BM25 results
- Vector results  
- Graph traversal results

Feed all three into RRF scoring. Chunks appearing in multiple lists rank highest.

**Step 5 — Pass merged context to LLM**
LLM gets richer, structurally connected context instead of just the 10 most similar chunks.

**Concrete changes needed:**
- `retriever.rs` — add `graph_search(entities: &[String]) -> Vec<String>` method that queries petgraph
- `api/mod.rs` — call `EntityExtractorTool` before `hybrid_search`, pass graph results into a 3-way RRF merge
- The petgraph snapshot already has the entity→chunk mappings from your Neo4j ingestion pipeline

This is essentially what Microsoft's GraphRAG paper describes, but you already have all the pieces — they just aren't connected at query time.

---------------

implement prio in hybrid rag between added docu's and llm.
---------------
screenresolution impl in frontend
--------------------------
