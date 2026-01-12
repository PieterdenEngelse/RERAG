What changes:
OFF: Raw text prompt. K/V recomputed each request.
ON: Structured messages. All providers cache K/V for matching prefixes.
Example (follow-up question):
Without: 5000 tokens computed twice
With: 5000 tokens cached, only new tokens computed
Per Backend:
    • Ollama: /api/chat + keep_alive
    • OpenAI: Structured messages for prefix caching
    • Anthropic: cache_control hints (beta)
✓ All backends supported


Reasons to Keep It Disabled by Default:
Reason	Explanation
	
Not universally beneficial	Short prompts (<1024 tokens) don't benefit from Anthropic caching
Resource usage	KV cache consumes GPU/CPU memory
Debugging simplicity	Stateless requests are easier to debug
Cost for cloud	Anthropic charges extra to write to cache
Cache misses	First request always has no benefit; varied prompts have low hit rates

Different API behavior	Ollama: /api/chat vs /api/generate have different semantics


When Users SHOULD Enable KV Cache:
Scenario	Why Enable
High-volume apps	Many similar requests benefit from cache reuse
Long system prompts	2000+ token system prompts get cached
RAG with stable context	Same documents retrieved repeatedly
Cost-sensitive production	Up to 10x cheaper on cloud API costs
Latency-sensitive	Up to 85% faster for long cached prompts
