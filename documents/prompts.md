Summary: RAG Memory Integration
New Features Added:
1. Memory Priority System (MemoryPriority enum)
unknown

Instruction (0) > Preference (1) > Persona (2) > Context (3) > Fact (4) > Summary (5) > Other (6)

2. Memory Type Mapping
Memory Type	Priority	Treatment
instruction	Highest	Directives to follow
preference	High	User preferences to respect
persona	Medium-High	Personality definitions
context	Medium	Background information
fact	Medium-Low	Factual information
summary, note	Low	Notes and summaries
Others	Lowest	General memories
3. Safety Filtering (is_safe_memory_content())

Rejects memories containing:

    Injection attempts: "ignore previous", "new instructions", "override", etc.
    Jailbreak patterns: "pretend you", "act as if", "you are now", etc.
    Suspicious content: Very long memories (>1000 chars), code blocks with "system"

4. Prompt Structure

The system prompt is now built with this hierarchy:
unknown

INSTRUCTIONS (follow these directives):
• [instruction memories]

PERSONA: [persona setting or persona memories]

USER PREFERENCES (respect these when responding):
• [preference memories]

FOCUS: [focus topic from /focus command]

RESPONSE STYLE: [verbosity from /verbose or /brief]

CONTEXT (background information):
• [context memories]

KNOWN FACTS:
• [fact memories]

NOTES:
• [summary/note memories - limited to 3]

5. Integration Points

    All agent endpoints (/agent, /agent/chat, /agent/stream) now load memories
    Memories are loaded from rag_memory table (limit: 20 most recent)
    Safety filter applied before injection

How Conflicts Are Resolved:

    Current conversation > Memories - The user's current query always takes precedence
    Settings > Memory-based settings - /focus, /persona, /verbose override memory-derived equivalents
    Instructions > Preferences > Facts - Higher priority memories appear first in prompt
    Safety filter > All memories - Suspicious content is silently dropped

