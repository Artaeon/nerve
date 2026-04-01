use super::SmartPrompt;

/// Helper to reduce boilerplate when defining built-in prompts.
fn p(
    name: &str,
    description: &str,
    template: &str,
    category: &str,
    tags: &[&str],
) -> SmartPrompt {
    SmartPrompt {
        name: name.into(),
        description: description.into(),
        template: template.into(),
        category: category.into(),
        tags: tags.iter().map(|s| (*s).into()).collect(),
    }
}

/// Return the comprehensive set of built-in SmartPrompts shipped with Nerve.
pub fn builtin_prompts() -> Vec<SmartPrompt> {
    vec![
        // ── Writing (12) ────────────────────────────────────────────────────
        p(
            "Summarize",
            "Create a clear, concise summary of any text",
            "Provide a clear, concise summary of the following text. Focus on the key points and main ideas. Keep the summary to about one-third of the original length.\n\n{{input}}",
            "Writing",
            &["summary", "shorten", "digest"],
        ),
        p(
            "Expand",
            "Expand text with more detail and supporting points",
            "Expand the following text with additional detail, examples, and supporting points. Maintain the original tone and style while making the content richer and more comprehensive.\n\n{{input}}",
            "Writing",
            &["expand", "elaborate", "lengthen"],
        ),
        p(
            "Rewrite Formally",
            "Rewrite text in a formal, professional tone",
            "Rewrite the following text in a formal, professional tone suitable for business or academic contexts. Preserve the original meaning while elevating the register and removing any colloquialisms.\n\n{{input}}",
            "Writing",
            &["formal", "professional", "tone"],
        ),
        p(
            "Rewrite Casually",
            "Rewrite text in a casual, conversational tone",
            "Rewrite the following text in a casual, friendly, and conversational tone. Make it feel approachable and easy to read while keeping the core message intact.\n\n{{input}}",
            "Writing",
            &["casual", "informal", "conversational"],
        ),
        p(
            "Fix Grammar",
            "Correct grammar, spelling, and punctuation errors",
            "Carefully proofread the following text and fix all grammar, spelling, and punctuation errors. Only make corrections \u{2014} do not change the style, tone, or meaning. List each change you made at the end.\n\n{{input}}",
            "Writing",
            &["grammar", "spelling", "proofread"],
        ),
        p(
            "Improve Clarity",
            "Improve the clarity and readability of text",
            "Improve the clarity and readability of the following text. Simplify complex sentences, remove unnecessary jargon, improve logical flow, and ensure the message is easy to understand. Preserve the original meaning.\n\n{{input}}",
            "Writing",
            &["clarity", "readability", "simplify"],
        ),
        p(
            "Write Email Reply",
            "Draft a professional reply to an email",
            "Draft a professional, courteous reply to the following email. Match an appropriate tone (formal for business, friendly for colleagues). Keep it concise and actionable. If the email asks questions, address each one.\n\n{{input}}",
            "Writing",
            &["email", "reply", "professional"],
        ),
        p(
            "Create Outline",
            "Generate a structured outline for any topic",
            "Create a well-structured outline for the following topic or text. Use a hierarchical format with main sections, subsections, and key points. Include enough detail to serve as a writing guide.\n\n{{input}}",
            "Writing",
            &["outline", "structure", "plan"],
        ),
        p(
            "Generate Headlines",
            "Create multiple compelling headline options",
            "Generate 10 compelling headline options for the following content. Include a mix of styles: informative, curiosity-driven, how-to, and listicle formats. Each headline should be concise and attention-grabbing.\n\n{{input}}",
            "Writing",
            &["headlines", "titles", "copywriting"],
        ),
        p(
            "Proofread",
            "Thorough proofreading with tracked changes",
            "Perform a thorough proofread of the following text. Check for:\n- Spelling and typographical errors\n- Grammar and syntax issues\n- Punctuation mistakes\n- Inconsistent formatting or style\n- Awkward phrasing\n\nProvide the corrected text followed by a numbered list of every change made with a brief explanation.\n\n{{input}}",
            "Writing",
            &["proofread", "edit", "review"],
        ),
        p(
            "Simplify Language",
            "Rewrite text using simpler, more accessible language",
            "Rewrite the following text using simpler, more accessible language. Target a general audience with no specialized knowledge. Replace jargon with plain alternatives and break up long sentences.\n\n{{input}}",
            "Writing",
            &["simplify", "plain language", "accessible"],
        ),
        p(
            "Change Tone",
            "Rewrite text in a specified tone",
            "Rewrite the following text in the tone specified. If no tone is specified, rewrite it in a neutral, balanced tone. Preserve the factual content and key message.\n\nDesired tone: {{tone}}\n\nText:\n{{input}}",
            "Writing",
            &["tone", "rewrite", "style"],
        ),

        // ── Coding (12) ────────────────────────────────────────────────────
        p(
            "Explain Code",
            "Get a clear, step-by-step explanation of code",
            "Explain the following code in clear, step-by-step terms. Cover:\n1. The overall purpose of the code\n2. How each major section works\n3. Any important algorithms or patterns used\n4. The inputs and outputs\n\nUse plain language suitable for an intermediate developer.\n\n```\n{{input}}\n```",
            "Coding",
            &["explain", "understand", "walkthrough"],
        ),
        p(
            "Fix Bug",
            "Identify and fix bugs in code",
            "Analyze the following code and identify any bugs, logic errors, or potential issues. For each problem found:\n1. Describe the bug and why it occurs\n2. Explain the impact (crash, wrong output, etc.)\n3. Provide the corrected code\n4. Explain the fix\n\n```\n{{input}}\n```",
            "Coding",
            &["debug", "fix", "bug"],
        ),
        p(
            "Refactor",
            "Refactor code for better design and readability",
            "Refactor the following code to improve its design, readability, and maintainability. Apply relevant best practices such as:\n- Single Responsibility Principle\n- DRY (Don't Repeat Yourself)\n- Meaningful variable and function names\n- Proper error handling\n- Reduced complexity\n\nProvide the refactored code and explain each change.\n\n```\n{{input}}\n```",
            "Coding",
            &["refactor", "clean code", "design"],
        ),
        p(
            "Add Comments",
            "Add clear documentation comments to code",
            "Add clear, useful documentation comments to the following code. Include:\n- Module/file-level documentation explaining the purpose\n- Function/method doc comments with parameter and return descriptions\n- Inline comments for non-obvious logic\n- Any relevant usage examples\n\nFollow the language's standard documentation conventions.\n\n```\n{{input}}\n```",
            "Coding",
            &["comments", "documentation", "docstring"],
        ),
        p(
            "Write Tests",
            "Generate comprehensive unit tests for code",
            "Write comprehensive unit tests for the following code. Include:\n- Happy path tests for normal operation\n- Edge case tests (empty inputs, boundary values, etc.)\n- Error case tests (invalid inputs, failure modes)\n- Use descriptive test names that explain the scenario\n\nUse the appropriate testing framework for the language.\n\n```\n{{input}}\n```",
            "Coding",
            &["tests", "unit test", "testing"],
        ),
        p(
            "Convert Language",
            "Convert code from one programming language to another",
            "Convert the following code to {{target_language}}. Produce idiomatic code in the target language \u{2014} don't just do a literal translation. Use the target language's conventions, standard library, and best practices. Note any features that don't have a direct equivalent.\n\nTarget language: {{target_language}}\n\n```\n{{input}}\n```",
            "Coding",
            &["convert", "translate", "port"],
        ),
        p(
            "Optimize Performance",
            "Optimize code for better performance",
            "Analyze the following code for performance issues and optimize it. Consider:\n- Time complexity improvements\n- Memory usage optimization\n- Unnecessary allocations or copies\n- Better data structures or algorithms\n- Caching opportunities\n\nProvide the optimized code and explain each improvement with its expected impact.\n\n```\n{{input}}\n```",
            "Coding",
            &["optimize", "performance", "speed"],
        ),
        p(
            "Code Review",
            "Perform a thorough code review",
            "Perform a thorough code review of the following code. Evaluate:\n- Correctness: Are there any bugs or logic errors?\n- Design: Is the code well-structured and maintainable?\n- Performance: Are there any inefficiencies?\n- Security: Are there any vulnerabilities?\n- Readability: Is the code clear and well-documented?\n- Best practices: Does it follow language conventions?\n\nProvide specific, actionable feedback with severity ratings (critical/major/minor/suggestion).\n\n```\n{{input}}\n```",
            "Coding",
            &["review", "feedback", "quality"],
        ),
        p(
            "Generate Docs",
            "Generate API or module documentation",
            "Generate comprehensive documentation for the following code. Include:\n- Overview and purpose\n- Public API reference with types, parameters, and return values\n- Usage examples for each public function/method\n- Any important notes about error handling or edge cases\n\nFormat the documentation in Markdown.\n\n```\n{{input}}\n```",
            "Coding",
            &["docs", "documentation", "api"],
        ),
        p(
            "Explain Error",
            "Explain an error message and how to fix it",
            "Explain the following error message in plain language. Cover:\n1. What the error means\n2. Common causes of this error\n3. Step-by-step instructions to fix it\n4. How to prevent it in the future\n\nIf code context is provided, give a specific fix for that context.\n\n{{input}}",
            "Coding",
            &["error", "debug", "troubleshoot"],
        ),
        p(
            "Write Function",
            "Generate a function from a description",
            "Write a function based on the following description. Include:\n- Clear function signature with typed parameters and return value\n- Input validation\n- Proper error handling\n- Documentation comments\n- At least two usage examples\n\nDescription:\n{{input}}",
            "Coding",
            &["generate", "function", "implement"],
        ),
        p(
            "Regex Help",
            "Create or explain regular expressions",
            "Help with the following regular expression task. If a regex is provided, explain it in detail. If a description is provided, write the regex.\n\nFor explanations, break down each part of the pattern. For new regexes, provide:\n- The regex pattern\n- An explanation of each component\n- Example matches and non-matches\n- Any caveats or limitations\n\n{{input}}",
            "Coding",
            &["regex", "pattern", "match"],
        ),

        // ── Translation (8) ────────────────────────────────────────────────
        p(
            "Translate to English",
            "Translate text into English",
            "Translate the following text into natural, fluent English. Preserve the original meaning, tone, and nuance as closely as possible. If any phrases are culturally specific, provide a brief explanation in parentheses.\n\n{{input}}",
            "Translation",
            &["english", "translate"],
        ),
        p(
            "Translate to Spanish",
            "Translate text into Spanish",
            "Translate the following text into natural, fluent Spanish. Use standard Latin American Spanish unless otherwise specified. Preserve the original meaning, tone, and nuance.\n\n{{input}}",
            "Translation",
            &["spanish", "translate"],
        ),
        p(
            "Translate to French",
            "Translate text into French",
            "Translate the following text into natural, fluent French. Preserve the original meaning, tone, and nuance. Use standard metropolitan French.\n\n{{input}}",
            "Translation",
            &["french", "translate"],
        ),
        p(
            "Translate to German",
            "Translate text into German",
            "Translate the following text into natural, fluent German. Preserve the original meaning, tone, and nuance. Use standard High German (Hochdeutsch).\n\n{{input}}",
            "Translation",
            &["german", "translate"],
        ),
        p(
            "Translate to Japanese",
            "Translate text into Japanese",
            "Translate the following text into natural Japanese. Use an appropriate level of formality based on context (default to polite/desu-masu form). Include furigana for uncommon kanji in parentheses.\n\n{{input}}",
            "Translation",
            &["japanese", "translate"],
        ),
        p(
            "Translate to Arabic",
            "Translate text into Arabic",
            "Translate the following text into natural, fluent Modern Standard Arabic. Preserve the original meaning, tone, and nuance.\n\n{{input}}",
            "Translation",
            &["arabic", "translate"],
        ),
        p(
            "Translate to Chinese",
            "Translate text into Simplified Chinese",
            "Translate the following text into natural, fluent Simplified Chinese (Mandarin). Preserve the original meaning, tone, and nuance.\n\n{{input}}",
            "Translation",
            &["chinese", "mandarin", "translate"],
        ),
        p(
            "Translate to Portuguese",
            "Translate text into Portuguese",
            "Translate the following text into natural, fluent Brazilian Portuguese. Preserve the original meaning, tone, and nuance.\n\n{{input}}",
            "Translation",
            &["portuguese", "translate"],
        ),

        // ── Analysis (10) ───────────────────────────────────────────────────
        p(
            "Analyze Sentiment",
            "Analyze the sentiment and emotional tone of text",
            "Analyze the sentiment and emotional tone of the following text. Provide:\n1. Overall sentiment (positive, negative, neutral, mixed)\n2. Confidence level (high, medium, low)\n3. Key emotional tones detected (e.g., joy, frustration, urgency)\n4. Specific phrases that indicate the sentiment\n5. A brief summary of the emotional landscape\n\n{{input}}",
            "Analysis",
            &["sentiment", "emotion", "tone"],
        ),
        p(
            "Extract Key Points",
            "Extract the main points and takeaways from text",
            "Extract the key points and main takeaways from the following text. Present them as:\n1. A numbered list of main points (in order of importance)\n2. Supporting details for each point\n3. A one-sentence overall takeaway\n\nBe thorough but concise \u{2014} capture everything important without adding interpretation.\n\n{{input}}",
            "Analysis",
            &["key points", "extract", "takeaways"],
        ),
        p(
            "Compare & Contrast",
            "Compare and contrast two or more items",
            "Provide a detailed comparison of the following items. Structure your analysis as:\n1. Overview of each item\n2. Similarities\n3. Differences (organized by category)\n4. Strengths and weaknesses of each\n5. Recommendation or conclusion (if applicable)\n\nUse a balanced, objective perspective.\n\n{{input}}",
            "Analysis",
            &["compare", "contrast", "versus"],
        ),
        p(
            "SWOT Analysis",
            "Perform a SWOT analysis",
            "Perform a comprehensive SWOT analysis of the following subject. For each quadrant, provide 3\u{2013}5 specific, actionable points:\n\n**Strengths** (internal positive factors)\n**Weaknesses** (internal negative factors)\n**Opportunities** (external positive factors)\n**Threats** (external negative factors)\n\nConclude with strategic recommendations based on the analysis.\n\n{{input}}",
            "Analysis",
            &["swot", "strategy", "business"],
        ),
        p(
            "Fact Check",
            "Verify claims and identify potential inaccuracies",
            "Analyze the following text for factual accuracy. For each claim or statement:\n1. Identify the specific claim\n2. Assess its accuracy (confirmed, likely true, uncertain, likely false, false)\n3. Provide reasoning or known context\n4. Note any claims that need additional verification\n\nBe transparent about the limits of your knowledge and clearly distinguish between verified facts and assessments.\n\n{{input}}",
            "Analysis",
            &["fact check", "verify", "accuracy"],
        ),
        p(
            "Identify Bias",
            "Identify potential biases in text",
            "Analyze the following text for potential biases. Look for:\n1. Loaded or emotionally charged language\n2. One-sided presentation of issues\n3. Omission of relevant perspectives\n4. Generalizations or stereotypes\n5. Selection bias in examples or evidence\n6. Framing effects\n\nProvide specific examples from the text and suggest how it could be made more balanced.\n\n{{input}}",
            "Analysis",
            &["bias", "objectivity", "critical thinking"],
        ),
        p(
            "Root Cause Analysis",
            "Identify the root cause of a problem",
            "Perform a root cause analysis of the following problem. Use the following approach:\n1. Clearly define the problem\n2. Apply the '5 Whys' technique to dig deeper\n3. Identify contributing factors\n4. Determine the root cause(s)\n5. Propose corrective actions\n6. Suggest preventive measures\n\n{{input}}",
            "Analysis",
            &["root cause", "problem solving", "5 whys"],
        ),
        p(
            "Data Interpretation",
            "Interpret data and identify patterns",
            "Analyze and interpret the following data. Provide:\n1. Summary of what the data shows\n2. Key patterns, trends, or anomalies\n3. Statistical observations (if applicable)\n4. Possible explanations for the patterns\n5. Limitations of the data\n6. Recommendations for further analysis\n\n{{input}}",
            "Analysis",
            &["data", "statistics", "patterns"],
        ),
        p(
            "Explain Concept",
            "Explain a concept clearly and thoroughly",
            "Explain the following concept in clear, accessible terms. Structure your explanation as:\n1. Simple one-sentence definition\n2. Detailed explanation with context\n3. A real-world analogy or example\n4. Why it matters / practical applications\n5. Common misconceptions\n6. Related concepts to explore\n\n{{input}}",
            "Analysis",
            &["explain", "concept", "learn"],
        ),
        p(
            "Pros and Cons",
            "List the pros and cons of something",
            "Provide a balanced, thorough list of pros and cons for the following. For each point:\n- Be specific, not generic\n- Explain the reasoning\n- Rate the significance (high/medium/low)\n\nEnd with an overall assessment and any caveats that depend on specific circumstances.\n\n{{input}}",
            "Analysis",
            &["pros", "cons", "advantages", "disadvantages"],
        ),

        // ── Creative (9) ────────────────────────────────────────────────────
        p(
            "Brainstorm Ideas",
            "Generate creative ideas for any topic",
            "Brainstorm a diverse set of creative ideas for the following topic. Generate at least 10 ideas ranging from conventional to unconventional. For each idea:\n- Give it a catchy title\n- Provide a 1\u{2013}2 sentence description\n- Rate its feasibility (easy/medium/hard)\n\nDon't self-censor \u{2014} include ambitious and unconventional ideas alongside practical ones.\n\n{{input}}",
            "Creative",
            &["brainstorm", "ideas", "creative"],
        ),
        p(
            "Write Story",
            "Create a short story or narrative",
            "Write a compelling short story based on the following prompt or theme. Include:\n- Vivid characters with distinct voices\n- A clear narrative arc (setup, conflict, resolution)\n- Sensory details and engaging prose\n- Dialogue where appropriate\n\nAim for approximately 500\u{2013}800 words unless otherwise specified.\n\n{{input}}",
            "Creative",
            &["story", "fiction", "narrative"],
        ),
        p(
            "Create Metaphor",
            "Generate metaphors and analogies for concepts",
            "Create a set of vivid, illuminating metaphors and analogies for the following concept. Provide:\n1. 3\u{2013}5 metaphors or analogies, each from a different domain (nature, technology, everyday life, etc.)\n2. A brief explanation of how each metaphor maps to the concept\n3. Which metaphor works best for different audiences\n\nMake them memorable and easy to understand.\n\n{{input}}",
            "Creative",
            &["metaphor", "analogy", "figurative"],
        ),
        p(
            "Generate Names",
            "Generate creative name ideas",
            "Generate creative name ideas for the following. Provide:\n- 15\u{2013}20 name suggestions organized by style (professional, playful, abstract, descriptive, etc.)\n- A brief note on the feeling each name evokes\n- Domain/trademark considerations (if applicable)\n- Your top 3 recommendations with reasoning\n\n{{input}}",
            "Creative",
            &["names", "naming", "branding"],
        ),
        p(
            "Write Poem",
            "Compose a poem on any topic",
            "Write a poem based on the following theme or subject. Create something with:\n- Vivid imagery and sensory language\n- A consistent rhythm or structure\n- Emotional resonance\n- Original metaphors\n\nFeel free to choose the form (free verse, sonnet, haiku, etc.) that best fits the subject, or specify a preferred form.\n\n{{input}}",
            "Creative",
            &["poem", "poetry", "verse"],
        ),
        p(
            "Create Slogan",
            "Craft catchy slogans and taglines",
            "Create 10 catchy slogans or taglines for the following. Each should be:\n- Memorable and concise (under 10 words)\n- Emotionally resonant\n- Easy to say aloud\n- Unique and not a cliche\n\nInclude a mix of styles: witty, inspiring, direct, and playful. Mark your top 3 picks.\n\n{{input}}",
            "Creative",
            &["slogan", "tagline", "marketing"],
        ),
        p(
            "Role Play",
            "Engage in a role-play scenario",
            "Let's engage in a role-play scenario. I'll describe the situation and the role I'd like you to play. Stay in character throughout, responding as that person or entity would. Be authentic to the character's knowledge, personality, and communication style.\n\nScenario:\n{{input}}",
            "Creative",
            &["roleplay", "character", "simulation"],
        ),
        p(
            "Devil's Advocate",
            "Argue the opposing perspective",
            "Play devil's advocate for the following position or idea. Construct the strongest possible counter-arguments by:\n1. Identifying the weakest assumptions in the position\n2. Presenting evidence or reasoning that challenges each point\n3. Offering alternative explanations or viewpoints\n4. Highlighting potential unintended consequences\n\nBe rigorous and fair \u{2014} the goal is to stress-test the idea, not to be contrarian.\n\n{{input}}",
            "Creative",
            &["devil's advocate", "counter", "debate"],
        ),
        p(
            "Rewrite Creatively",
            "Rewrite text with creative flair",
            "Rewrite the following text with creative flair and engaging style. Inject personality, vivid language, and a compelling rhythm while preserving the core message. Make it a pleasure to read.\n\n{{input}}",
            "Creative",
            &["creative writing", "style", "flair"],
        ),

        // ── Productivity (10) ──────────────────────────────────────────────
        p(
            "Create Action Items",
            "Extract actionable tasks from text",
            "Extract clear, specific action items from the following text. For each action item:\n- Write it as a concrete, actionable task (start with a verb)\n- Assign a priority (high/medium/low)\n- Note any deadlines mentioned\n- Identify the responsible party (if mentioned)\n\nOrganize by priority and group related items together.\n\n{{input}}",
            "Productivity",
            &["action items", "tasks", "todo"],
        ),
        p(
            "Meeting Summary",
            "Summarize meeting notes or transcripts",
            "Create a structured meeting summary from the following notes or transcript. Include:\n\n**Attendees:** (if mentioned)\n**Date:** (if mentioned)\n**Key Discussion Points:** (numbered list)\n**Decisions Made:** (numbered list)\n**Action Items:** (with owners and deadlines)\n**Open Questions:** (items needing follow-up)\n**Next Steps:** (what happens next)\n\n{{input}}",
            "Productivity",
            &["meeting", "summary", "notes"],
        ),
        p(
            "Decision Matrix",
            "Create a structured decision matrix",
            "Create a decision matrix to help evaluate the following options. Steps:\n1. Identify the key criteria for the decision\n2. Weight each criterion by importance (1\u{2013}5)\n3. Score each option against each criterion (1\u{2013}5)\n4. Calculate weighted scores\n5. Present results in a clear table\n6. Provide a recommendation with reasoning\n\n{{input}}",
            "Productivity",
            &["decision", "matrix", "evaluate"],
        ),
        p(
            "Prioritize Tasks",
            "Prioritize a list of tasks using proven frameworks",
            "Prioritize the following tasks using the Eisenhower Matrix (urgent/important framework). Categorize each task into:\n\n1. **Do First** (urgent + important)\n2. **Schedule** (important, not urgent)\n3. **Delegate** (urgent, not important)\n4. **Eliminate** (neither urgent nor important)\n\nProvide reasoning for each categorization and suggest an execution order.\n\n{{input}}",
            "Productivity",
            &["prioritize", "eisenhower", "tasks"],
        ),
        p(
            "Write Report",
            "Draft a professional report",
            "Draft a professional report based on the following information. Use this structure:\n\n1. **Executive Summary** (1 paragraph)\n2. **Background/Context**\n3. **Findings/Analysis** (main body)\n4. **Conclusions**\n5. **Recommendations**\n\nUse clear, professional language and support claims with the provided data.\n\n{{input}}",
            "Productivity",
            &["report", "professional", "business"],
        ),
        p(
            "Create Agenda",
            "Create a structured meeting agenda",
            "Create a well-structured meeting agenda based on the following topics and goals. Include:\n- Meeting title and objective\n- Time allocations for each item\n- Discussion leader for each topic (if applicable)\n- Required preparation or pre-reads\n- Space for Q&A and wrap-up\n\nTotal meeting time: {{duration}} (default: 60 minutes)\n\n{{input}}",
            "Productivity",
            &["agenda", "meeting", "planning"],
        ),
        p(
            "Elevator Pitch",
            "Craft a concise elevator pitch",
            "Craft a compelling elevator pitch for the following. The pitch should:\n- Be deliverable in 30\u{2013}60 seconds\n- Hook the listener in the first sentence\n- Clearly state the value proposition\n- Address the target audience's pain point\n- End with a call to action\n\nProvide three versions: formal, conversational, and bold.\n\n{{input}}",
            "Productivity",
            &["pitch", "elevator", "persuade"],
        ),
        p(
            "Project Brief",
            "Create a project brief document",
            "Create a comprehensive project brief based on the following information. Include:\n\n1. **Project Overview** (goals and scope)\n2. **Problem Statement** (what we're solving)\n3. **Target Audience / Stakeholders**\n4. **Success Criteria** (measurable outcomes)\n5. **Key Deliverables**\n6. **Timeline / Milestones**\n7. **Resources Needed**\n8. **Risks and Mitigation**\n9. **Out of Scope** (explicit exclusions)\n\n{{input}}",
            "Productivity",
            &["project", "brief", "planning"],
        ),
        p(
            "Draft Email",
            "Draft a professional email",
            "Draft a professional email based on the following context. Ensure the email:\n- Has a clear, specific subject line\n- Opens with appropriate context\n- States the purpose concisely\n- Includes any necessary details\n- Ends with a clear call to action\n- Uses an appropriate sign-off\n\n{{input}}",
            "Productivity",
            &["email", "professional", "communication"],
        ),
        p(
            "ELI5",
            "Explain like I'm five years old",
            "Explain the following concept as if you were talking to a five-year-old. Use simple words, fun comparisons to everyday things kids understand, and keep it short and engaging. Avoid any technical jargon.\n\n{{input}}",
            "Productivity",
            &["eli5", "simple", "explain"],
        ),

        // ── Engineering (15) ───────────────────────────────────────────────
        p(
            "Architecture Review",
            "Review architecture for scalability and best practices",
            "Review the following architecture or design for scalability, maintainability, and best practices. Identify potential issues and suggest improvements.\n\n{{input}}",
            "Engineering",
            &["architecture", "scalability", "maintainability", "review"],
        ),
        p(
            "API Design",
            "Design or review an API with RESTful best practices",
            "Design or review the following API. Consider RESTful principles, naming conventions, versioning, error handling, pagination, and authentication. Provide a well-structured API specification.\n\n{{input}}",
            "Engineering",
            &["api", "rest", "design", "endpoints"],
        ),
        p(
            "Database Schema",
            "Design or review a database schema",
            "Design or review the following database schema. Consider normalization, indexing, relationships, constraints, and performance. Suggest improvements.\n\n{{input}}",
            "Engineering",
            &["database", "schema", "sql", "normalization"],
        ),
        p(
            "System Design",
            "Design a system from requirements",
            "Design a system for the following requirements. Consider scalability, reliability, availability, and performance. Include components, data flow, and trade-offs.\n\n{{input}}",
            "Engineering",
            &["system design", "architecture", "scalability", "distributed"],
        ),
        p(
            "Security Audit",
            "Audit code or architecture for security vulnerabilities",
            "Perform a security audit on the following code or architecture. Check for OWASP Top 10 vulnerabilities, authentication issues, injection risks, and data exposure. Provide remediation steps.\n\n{{input}}",
            "Engineering",
            &["security", "owasp", "audit", "vulnerabilities"],
        ),
        p(
            "Performance Optimize",
            "Analyze and optimize system or code performance",
            "Analyze the following code or system for performance bottlenecks. Consider time complexity, memory usage, I/O patterns, caching opportunities, and concurrency. Suggest optimizations with benchmarks.\n\n{{input}}",
            "Engineering",
            &["performance", "optimization", "bottleneck", "caching"],
        ),
        p(
            "Error Handling",
            "Review and improve error handling patterns",
            "Review and improve the error handling in the following code. Ensure proper error propagation, user-friendly messages, logging, retry logic, and graceful degradation.\n\n{{input}}",
            "Engineering",
            &["error handling", "resilience", "retry", "graceful degradation"],
        ),
        p(
            "Test Strategy",
            "Design a comprehensive test strategy",
            "Design a comprehensive test strategy for the following code or feature. Include unit tests, integration tests, edge cases, error scenarios, and property-based test suggestions.\n\n{{input}}",
            "Engineering",
            &["testing", "strategy", "unit test", "integration test"],
        ),
        p(
            "Code Migration",
            "Plan a codebase migration strategy",
            "Plan a migration strategy for the following codebase change. Consider backward compatibility, feature flags, rollback plan, data migration, and deployment strategy.\n\n{{input}}",
            "Engineering",
            &["migration", "backward compatibility", "rollback", "deployment"],
        ),
        p(
            "Dependency Audit",
            "Audit dependencies for health and security",
            "Audit the following dependency list or code for dependency health. Check for outdated packages, security vulnerabilities, license compatibility, and unnecessary dependencies. Suggest alternatives.\n\n{{input}}",
            "Engineering",
            &["dependencies", "audit", "security", "licenses"],
        ),
        p(
            "Write Dockerfile",
            "Create an optimized Dockerfile",
            "Create an optimized Dockerfile for the following application. Use multi-stage builds, minimize layers, follow security best practices, and optimize for cache efficiency.\n\n{{input}}",
            "Engineering",
            &["docker", "dockerfile", "containerization", "devops"],
        ),
        p(
            "CI/CD Pipeline",
            "Design a CI/CD pipeline for a project",
            "Design a CI/CD pipeline for the following project. Include build, test, lint, security scan, deploy stages. Consider parallelism, caching, and rollback strategies.\n\n{{input}}",
            "Engineering",
            &["ci/cd", "pipeline", "automation", "deployment"],
        ),
        p(
            "Debug Systematically",
            "Systematically debug an issue",
            "Help me debug the following issue systematically. Walk through potential causes from most to least likely, suggest diagnostic steps, and propose fixes for each scenario.\n\n{{input}}",
            "Engineering",
            &["debug", "systematic", "diagnostics", "troubleshoot"],
        ),
        p(
            "Write Documentation",
            "Write comprehensive technical documentation",
            "Write comprehensive technical documentation for the following code, API, or system. Include overview, setup, usage examples, configuration reference, and troubleshooting guide.\n\n{{input}}",
            "Engineering",
            &["documentation", "technical writing", "reference", "guide"],
        ),
        p(
            "Microservice Design",
            "Design a microservice with clear boundaries",
            "Design a microservice for the following requirements. Define boundaries, API contracts, data ownership, inter-service communication patterns, and failure handling.\n\n{{input}}",
            "Engineering",
            &["microservice", "api contract", "service boundary", "distributed"],
        ),

        // ── Design (10) ────────────────────────────────────────────────────
        p(
            "UX Review",
            "Review a UI or flow for usability issues",
            "Review the following user interface or flow for usability issues. Consider accessibility (WCAG), information hierarchy, cognitive load, and user journey. Suggest improvements.\n\n{{input}}",
            "Design",
            &["ux", "usability", "accessibility", "user journey"],
        ),
        p(
            "Component Design",
            "Design a reusable UI component",
            "Design a reusable UI component for the following requirements. Include props/API, states, variants, accessibility, responsive behavior, and usage examples.\n\n{{input}}",
            "Design",
            &["component", "ui", "reusable", "props"],
        ),
        p(
            "Color Palette",
            "Create a cohesive color palette",
            "Create a cohesive color palette for the following project or brand. Include primary, secondary, accent, background, and text colors with hex codes. Ensure WCAG AA contrast compliance.\n\n{{input}}",
            "Design",
            &["color", "palette", "branding", "contrast"],
        ),
        p(
            "Typography System",
            "Design a typography system",
            "Design a typography system for the following project. Include font families, size scale, line heights, font weights, and usage guidelines for headings, body, captions, and code.\n\n{{input}}",
            "Design",
            &["typography", "fonts", "type scale", "readability"],
        ),
        p(
            "Design System",
            "Create a design system specification",
            "Create a design system specification for the following product. Include design tokens, component library structure, naming conventions, and documentation standards.\n\n{{input}}",
            "Design",
            &["design system", "tokens", "component library", "standards"],
        ),
        p(
            "Wireframe Description",
            "Create a detailed wireframe description",
            "Create a detailed wireframe description for the following page or feature. Describe layout, component placement, content hierarchy, interactions, and responsive breakpoints.\n\n{{input}}",
            "Design",
            &["wireframe", "layout", "mockup", "responsive"],
        ),
        p(
            "Accessibility Audit",
            "Audit UI or HTML for accessibility compliance",
            "Audit the following UI or HTML for accessibility issues. Check WCAG 2.1 AA compliance, screen reader compatibility, keyboard navigation, color contrast, and ARIA attributes.\n\n{{input}}",
            "Design",
            &["accessibility", "wcag", "a11y", "screen reader"],
        ),
        p(
            "Animation Spec",
            "Design animation specifications for UI interactions",
            "Design animation specifications for the following UI interactions. Include timing, easing, duration, triggers, and fallbacks for reduced-motion preferences.\n\n{{input}}",
            "Design",
            &["animation", "motion", "easing", "interaction"],
        ),
        p(
            "Responsive Design",
            "Plan a responsive design strategy",
            "Plan a responsive design strategy for the following layout. Define breakpoints, layout shifts, component adaptations, and mobile-first considerations.\n\n{{input}}",
            "Design",
            &["responsive", "breakpoints", "mobile-first", "adaptive"],
        ),
        p(
            "Icon Design Brief",
            "Create an icon design brief",
            "Create an icon design brief for the following set of actions or concepts. Include style guidelines, size grid, stroke weight, visual metaphors, and consistency rules.\n\n{{input}}",
            "Design",
            &["icons", "iconography", "visual design", "consistency"],
        ),

        // ── Best Practices (10) ────────────────────────────────────────────
        p(
            "Code Standards",
            "Define coding standards and conventions",
            "Define coding standards and conventions for the following language or project. Include naming, formatting, file organization, documentation, error handling, and testing requirements.\n\n{{input}}",
            "Best Practices",
            &["standards", "conventions", "style guide", "formatting"],
        ),
        p(
            "PR Review Checklist",
            "Review a pull request against best practices",
            "Review the following pull request or code changes against best practices. Check for correctness, readability, test coverage, performance, security, and documentation. Provide actionable feedback.\n\n{{input}}",
            "Best Practices",
            &["pull request", "code review", "checklist", "feedback"],
        ),
        p(
            "Clean Code Refactor",
            "Refactor code to follow clean code principles",
            "Refactor the following code to follow clean code principles. Improve naming, reduce complexity, extract methods, remove duplication, and add clarity. Explain each change.\n\n{{input}}",
            "Best Practices",
            &["clean code", "refactor", "readability", "simplicity"],
        ),
        p(
            "SOLID Principles",
            "Analyze code for SOLID principle violations",
            "Analyze the following code or design for SOLID principle violations. Identify which principles are violated, explain why it matters, and show the refactored version.\n\n{{input}}",
            "Best Practices",
            &["solid", "design principles", "oop", "refactor"],
        ),
        p(
            "Naming Convention",
            "Review and improve naming in code",
            "Review and improve the naming in the following code. Apply consistent conventions, use descriptive names, and ensure clarity of intent for variables, functions, classes, and modules.\n\n{{input}}",
            "Best Practices",
            &["naming", "conventions", "readability", "clarity"],
        ),
        p(
            "Logging Strategy",
            "Design a logging strategy for an application",
            "Design a logging strategy for the following application. Define log levels, structured logging format, what to log, what NOT to log (PII/secrets), and monitoring integration.\n\n{{input}}",
            "Best Practices",
            &["logging", "monitoring", "observability", "structured logs"],
        ),
        p(
            "Config Management",
            "Design a configuration management strategy",
            "Design a configuration management strategy for the following application. Consider environment variables, config files, secrets management, validation, and defaults.\n\n{{input}}",
            "Best Practices",
            &["configuration", "env vars", "secrets", "management"],
        ),
        p(
            "Error Catalog",
            "Create an error catalog for an application or API",
            "Create an error catalog for the following application or API. Define error codes, messages, HTTP status codes, user-facing descriptions, and debugging hints.\n\n{{input}}",
            "Best Practices",
            &["error codes", "catalog", "api errors", "status codes"],
        ),
        p(
            "Tech Debt Assessment",
            "Assess technical debt in a codebase",
            "Assess the technical debt in the following codebase or system. Categorize issues by severity and effort, prioritize remediation, and estimate impact of each improvement.\n\n{{input}}",
            "Best Practices",
            &["tech debt", "assessment", "remediation", "prioritization"],
        ),
        p(
            "Onboarding Guide",
            "Create a developer onboarding guide",
            "Create a developer onboarding guide for the following project. Include architecture overview, setup steps, key conventions, common tasks, debugging tips, and resource links.\n\n{{input}}",
            "Best Practices",
            &["onboarding", "developer guide", "setup", "documentation"],
        ),

        // ── Git (8) ────────────────────────────────────────────────────────
        p(
            "Git Init & Commit",
            "Initialize a git repo with organized commit history",
            "Initialize a git repository for this project and create an organized commit history. Set up the author as specified by the user. Create a .gitignore appropriate for the project type. Make logical, atomic commits with clear messages. Do NOT add 'Co-Authored-By' lines to any commit messages.\n\n{{input}}",
            "Git",
            &["git init", "commit", "gitignore", "repository"],
        ),
        p(
            "Commit Message",
            "Write a conventional commit message",
            "Write a clear, conventional commit message for the following changes. Follow the Conventional Commits format (type(scope): description). Keep the subject under 72 characters. Add a body explaining the 'why' if the change is non-trivial.\n\n{{input}}",
            "Git",
            &["commit", "conventional commits", "message", "git"],
        ),
        p(
            "PR Description",
            "Write a pull request description",
            "Write a pull request description for the following changes. Include a summary of what changed and why, testing instructions, screenshots if applicable, and any breaking changes.\n\n{{input}}",
            "Git",
            &["pull request", "pr", "description", "review"],
        ),
        p(
            "Git Workflow",
            "Design a git branching strategy",
            "Design a git branching strategy and workflow for the following team or project. Consider branch naming, merge strategy, release process, hotfix procedure, and CI integration.\n\n{{input}}",
            "Git",
            &["branching", "workflow", "gitflow", "strategy"],
        ),
        p(
            "Changelog Entry",
            "Write a changelog entry for a release",
            "Write a changelog entry for the following release or changes. Follow Keep a Changelog format with Added, Changed, Deprecated, Removed, Fixed, and Security sections.\n\n{{input}}",
            "Git",
            &["changelog", "release", "keep a changelog", "versioning"],
        ),
        p(
            "Release Notes",
            "Write user-facing release notes",
            "Write user-facing release notes for the following changes. Highlight new features, improvements, and bug fixes in clear, non-technical language. Organize by importance.\n\n{{input}}",
            "Git",
            &["release notes", "user-facing", "features", "changelog"],
        ),
        p(
            "Git Troubleshoot",
            "Troubleshoot a git issue",
            "Help troubleshoot the following git issue. Explain what happened, why, and provide step-by-step resolution. Include commands with explanations.\n\n{{input}}",
            "Git",
            &["git", "troubleshoot", "resolve", "commands"],
        ),
        p(
            "Gitignore Template",
            "Create a comprehensive .gitignore file",
            "Create a comprehensive .gitignore file for the following project type and tech stack. Include common IDE files, build artifacts, dependency directories, environment files, and OS-specific files.\n\n{{input}}",
            "Git",
            &["gitignore", "ignore", "template", "project setup"],
        ),

        // ── Engineering — SmartPrompts ─────────────────────────────────────
        p(
            "Full Code Review",
            "Perform a thorough, senior-level code review with severity ratings",
            "You are a senior software engineer performing a thorough code review. Analyze the following code with extreme attention to detail.\n\nReview checklist:\n1. CORRECTNESS: Logic errors, off-by-one, null/undefined handling, race conditions\n2. SECURITY: Injection risks, auth issues, data exposure, OWASP Top 10\n3. PERFORMANCE: Time/space complexity, unnecessary allocations, N+1 queries, missing indexes\n4. READABILITY: Naming, structure, comments where non-obvious, dead code\n5. MAINTAINABILITY: SOLID principles, coupling, cohesion, testability\n6. ERROR HANDLING: Edge cases, graceful degradation, error messages\n7. TESTING: Missing test cases, edge cases not covered\n\nFor each issue found, provide:\n- Severity: CRITICAL / WARNING / SUGGESTION\n- Line reference or code snippet\n- What's wrong and why\n- Concrete fix with code example\n\nEnd with a summary: overall quality score (1-10), top 3 priorities to fix, and what's done well.\n\n{{input}}",
            "Engineering",
            &["code review", "quality", "security", "best practices", "audit"],
        ),
        p(
            "Architect Solution",
            "Design a production-ready architecture from requirements",
            "You are a principal software architect. Design a production-ready solution for the following requirements.\n\nYour design MUST include:\n\n1. SYSTEM OVERVIEW\n   - High-level architecture diagram (describe in text/ASCII)\n   - Component inventory with responsibilities\n   - Technology choices with justification\n\n2. DATA MODEL\n   - Entity definitions with fields and types\n   - Relationships and cardinality\n   - Indexing strategy\n   - Migration considerations\n\n3. API DESIGN\n   - Endpoint inventory (method, path, request/response)\n   - Authentication and authorization scheme\n   - Rate limiting and pagination strategy\n   - Versioning approach\n\n4. SCALABILITY\n   - Expected load and growth projections\n   - Horizontal vs vertical scaling strategy\n   - Caching layers (what, where, TTL)\n   - Database scaling (read replicas, sharding)\n\n5. RELIABILITY\n   - Failure modes and mitigation\n   - Circuit breakers and retry policies\n   - Monitoring and alerting\n   - Disaster recovery and backup strategy\n\n6. TRADE-OFFS\n   - Key decisions and alternatives considered\n   - What was sacrificed and why\n   - Technical debt accepted and payoff plan\n\n{{input}}",
            "Engineering",
            &["architecture", "system design", "scalability", "production", "trade-offs"],
        ),
        p(
            "Debug Detective",
            "Systematically diagnose and fix issues like a senior debugger",
            "You are a senior debugging specialist. Help me systematically diagnose and fix this issue.\n\nFollow this structured debugging process:\n\nSTEP 1 - UNDERSTAND THE SYMPTOM\n- What exactly is happening vs what should happen?\n- When did it start? What changed recently?\n- Is it reproducible? Under what conditions?\n\nSTEP 2 - FORM HYPOTHESES (rank by likelihood)\n- List 5-7 possible root causes, most likely first\n- For each: what evidence would confirm or eliminate it?\n\nSTEP 3 - DIAGNOSTIC PLAN\n- Exact commands, queries, or code changes to run\n- What to look for in logs, metrics, or output\n- Binary search strategy to narrow down the cause\n\nSTEP 4 - ROOT CAUSE ANALYSIS\n- Based on evidence, identify the most likely cause\n- Explain the causal chain (A caused B which caused C)\n- Why existing tests/monitoring didn't catch this\n\nSTEP 5 - FIX\n- Minimal, targeted fix with code\n- Regression test to prevent recurrence\n- Related areas to check for the same class of bug\n\nSTEP 6 - PREVENTION\n- What process/tooling change prevents this category of bug?\n- Monitoring/alerting to catch it earlier next time\n\nHere's the issue:\n\n{{input}}",
            "Engineering",
            &["debug", "diagnose", "root cause", "troubleshoot", "systematic"],
        ),
        p(
            "Write Production Code",
            "Write clean, robust, production-quality code with tests",
            "You are a senior software engineer writing production-quality code. Write clean, robust, well-tested code for the following requirement.\n\nRequirements for your code:\n- Handle ALL edge cases and error conditions\n- Follow the language's idiomatic conventions and best practices\n- Use meaningful variable and function names\n- Add comments ONLY where the logic isn't self-evident\n- Include proper error types and error messages\n- No unnecessary abstractions \u{2014} keep it simple and direct\n- Consider performance for the expected scale\n- Thread-safe if applicable\n\nAfter the implementation, provide:\n1. The complete code\n2. Unit tests covering: happy path, edge cases, error conditions\n3. Brief explanation of key design decisions\n4. Any assumptions made\n\n{{input}}",
            "Engineering",
            &["production", "robust", "code", "implementation", "tests"],
        ),
        p(
            "Deep Performance Optimization",
            "Deep performance analysis and optimization with prioritized improvements",
            "You are a performance engineering specialist. Analyze and optimize the following code or system for maximum performance.\n\nYour analysis must cover:\n\n1. PROFILING ANALYSIS\n   - Identify the hot paths and bottlenecks\n   - Time complexity of critical operations\n   - Memory allocation patterns and pressure\n   - I/O wait and blocking operations\n\n2. QUICK WINS (< 1 hour to implement)\n   - Algorithm improvements\n   - Caching opportunities\n   - Unnecessary work elimination\n   - Better data structures\n\n3. MEDIUM EFFORT (1 day to implement)\n   - Architectural changes\n   - Concurrency/parallelism opportunities\n   - Batch processing optimizations\n   - Connection pooling, lazy loading\n\n4. STRATEGIC IMPROVEMENTS (1 week+)\n   - Redesign proposals\n   - Infrastructure changes\n   - Pre-computation and denormalization\n\nFor each optimization, provide:\n- Expected improvement (2x, 10x, etc.)\n- Implementation complexity\n- Risk of regression\n- Code example\n\n{{input}}",
            "Engineering",
            &["performance", "optimization", "profiling", "bottleneck", "scalability"],
        ),

        // ── Engineering — Scaffold & Feature Prompts ──────────────────────
        p(
            "Scaffold Project",
            "Generate a complete, production-ready project from requirements",
            "You are a senior software architect creating a production-ready project from scratch.\n\nCreate a complete, well-structured project for the following requirements. Your project MUST include:\n\nPROJECT STRUCTURE:\n- Logical directory organization following language conventions\n- Separation of concerns (routes, handlers, models, config, utils)\n- Environment-based configuration (.env.example, not .env)\n\nCODE QUALITY:\n- Proper error handling throughout (no unwrap/panic in Rust, no unhandled promises in JS)\n- Input validation on all external boundaries\n- Consistent naming conventions\n- Type safety where the language supports it\n\nCONFIGURATION:\n- Build/package file with all dependencies pinned to specific versions\n- Development and production configurations\n- Environment variable support for secrets\n\nTESTING:\n- Unit tests for core logic\n- Integration test for the main flow\n- Test configuration (test runner, fixtures)\n\nDOCUMENTATION:\n- README.md with: description, prerequisites, installation, usage, API reference, development guide\n- Inline comments for non-obvious logic\n- API documentation if applicable\n\nDEVOPS:\n- .gitignore appropriate for the language and framework\n- Dockerfile (multi-stage build, non-root user)\n- Basic CI config (.github/workflows/ci.yml)\n\nDo NOT use placeholder comments like '// TODO' or '// implement later'. Every file must contain real, working code.\n\nRequirements:\n\n{{input}}",
            "Engineering",
            &["scaffold", "project", "generate", "boilerplate", "new project"],
        ),
        p(
            "Add Feature",
            "Add a feature to an existing codebase following existing patterns",
            "You are a senior developer adding a feature to an existing codebase.\n\nBefore writing code, analyze:\n1. Existing code structure and patterns\n2. How similar features are implemented\n3. What files need to be created or modified\n4. What tests need to be added or updated\n\nThen implement the feature following these rules:\n- Match the existing code style exactly\n- Add the minimum code necessary\n- Include proper error handling\n- Add or update tests\n- Update documentation if needed\n- Do NOT refactor unrelated code\n\nProvide:\n1. List of files to create/modify\n2. The complete code for each file\n3. New tests\n4. Any migration or setup steps needed\n\nFeature request:\n\n{{input}}",
            "Engineering",
            &["feature", "add", "implement", "extend", "codebase"],
        ),
        p(
            "Refactor Code",
            "Refactor code without changing external behavior using a structured checklist",
            "You are a refactoring specialist. Improve the following code without changing its external behavior.\n\nRefactoring checklist:\n1. NAMING: Rename unclear variables, functions, and types to be self-documenting\n2. EXTRACTION: Break large functions into focused, single-responsibility units\n3. SIMPLIFICATION: Replace complex conditionals with early returns or pattern matching\n4. DUPLICATION: Identify and eliminate duplicated logic\n5. ABSTRACTIONS: Introduce interfaces/traits only if they reduce complexity (not for hypothetical future use)\n6. ERROR HANDLING: Replace generic errors with specific, actionable ones\n7. TYPES: Strengthen type safety (use newtypes, enums over strings, Option over null)\n8. DEAD CODE: Remove unused imports, variables, functions, and commented-out code\n\nFor EACH change:\n- What you changed\n- Why it's better\n- Risk level (safe/low/medium)\n\nShow the complete refactored code, not just snippets.\n\nCode to refactor:\n\n{{input}}",
            "Engineering",
            &["refactor", "improve", "clean", "restructure", "simplify"],
        ),
        p(
            "Comprehensive Test Suite",
            "Write thorough, maintainable tests covering all edge cases",
            "You are a testing expert. Write thorough, maintainable tests for the following code.\n\nTest categories to cover:\n1. HAPPY PATH: Normal usage with valid inputs\n2. EDGE CASES: Empty inputs, boundary values, maximum lengths\n3. ERROR CASES: Invalid inputs, missing data, network failures\n4. SECURITY: Injection attempts, unauthorized access, malformed data\n5. CONCURRENCY: Race conditions, deadlocks (if applicable)\n6. REGRESSION: Specific bugs that were fixed (if mentioned)\n\nFor each test:\n- Descriptive name that explains what's being tested\n- Arrange-Act-Assert pattern\n- One assertion per test (prefer many focused tests over few large ones)\n- Independent (no test depends on another test's state)\n\nInclude:\n- Test fixtures/helpers if patterns repeat\n- Mock/stub setup for external dependencies\n- Comments explaining non-obvious test logic\n\nCode to test:\n\n{{input}}",
            "Engineering",
            &["tests", "testing", "coverage", "edge cases", "quality assurance"],
        ),

        // ── Writing — SmartPrompts ─────────────────────────────────────────
        p(
            "Technical Blog Post",
            "Write an engaging, educational technical blog post",
            "You are a technical writer creating an engaging, educational blog post. Write a complete blog post on the following topic.\n\nStructure:\n1. HOOK \u{2014} Opening paragraph that grabs attention with a relatable problem or surprising fact\n2. CONTEXT \u{2014} Why this matters, who it's for, what they'll learn\n3. MAIN CONTENT \u{2014} Clear, logical progression with:\n   - Code examples that actually work (not pseudocode)\n   - Diagrams described in text where helpful\n   - Common pitfalls and how to avoid them\n   - Real-world use cases\n4. PRACTICAL TAKEAWAY \u{2014} Actionable next steps the reader can do today\n5. CONCLUSION \u{2014} Key points summarized, call to action\n\nWriting guidelines:\n- Write for a developer audience (intermediate level)\n- Use short paragraphs (3-4 sentences max)\n- Include section headers for scanability\n- Code examples should be complete and runnable\n- Avoid jargon without explanation\n- Aim for 1500-2500 words\n\nTopic:\n\n{{input}}",
            "Writing",
            &["blog", "technical writing", "tutorial", "educational", "content"],
        ),
        p(
            "Professional Email",
            "Draft a concise, action-oriented professional email",
            "You are an executive communications specialist. Draft a professional email that achieves its objective clearly and efficiently.\n\nBefore writing, analyze:\n- Who is the recipient and what's their context?\n- What is the ONE key action or takeaway?\n- What's the appropriate tone (formal, warm-professional, direct)?\n\nEmail structure:\n1. SUBJECT LINE \u{2014} Specific, action-oriented, under 60 characters\n2. OPENING \u{2014} Context in 1-2 sentences (why you're writing)\n3. BODY \u{2014} Key information, organized with bullets if 3+ points\n4. ASK \u{2014} Clear, specific call to action with timeline\n5. CLOSE \u{2014} Professional, appropriate warmth\n\nRules:\n- Keep under 200 words (executives skim)\n- One email = one topic\n- Bold or bullet the key ask if buried in context\n- Include any necessary context the recipient needs to act\n- Suggest specific times/dates rather than 'soon' or 'when convenient'\n\nContext for the email:\n\n{{input}}",
            "Writing",
            &["email", "professional", "business", "communication", "executive"],
        ),
        p(
            "Long-Form Article",
            "Write a comprehensive, authoritative long-form article",
            "You are an expert writer creating a comprehensive, well-researched article. Write a thorough, authoritative piece on the following topic.\n\nYour article must:\n- Open with a compelling hook that establishes why this matters NOW\n- Build a clear narrative arc (problem \u{2192} context \u{2192} analysis \u{2192} solution \u{2192} future)\n- Support every claim with specific evidence, data, or examples\n- Include expert-level insights that go beyond surface-level treatment\n- Address counterarguments and nuance\n- Use analogies and concrete examples to explain complex concepts\n- End with actionable insights and a forward-looking perspective\n\nFormatting:\n- Use clear section headers (H2, H3) for structure\n- Short paragraphs (3-5 sentences)\n- Pull quotes or callout boxes for key insights\n- Transition sentences between sections\n- 2000-4000 words\n\nTone: Authoritative but accessible. Write like a respected industry expert explaining to smart colleagues.\n\nTopic:\n\n{{input}}",
            "Writing",
            &["article", "long-form", "authoritative", "in-depth", "research"],
        ),

        // ── Data (NEW category) ────────────────────────────────────────────
        p(
            "SQL Query Builder",
            "Write optimized SQL queries with explanations and index advice",
            "You are a database expert. Write an optimized SQL query for the following requirement.\n\nYour response must include:\n1. The SQL query with clear formatting and comments\n2. Explanation of the query logic step by step\n3. Expected execution plan considerations\n4. Index recommendations for optimal performance\n5. Alternative approaches if applicable\n6. Edge cases the query handles (NULLs, empty results, duplicates)\n\nGuidelines:\n- Use standard SQL (note any vendor-specific syntax)\n- Prefer CTEs over subqueries for readability\n- Include appropriate JOIN types with reasoning\n- Add WHERE clause optimizations\n- Consider query plan: avoid full table scans, use index-friendly predicates\n- Handle NULL values explicitly\n\nRequirement:\n\n{{input}}",
            "Data",
            &["sql", "database", "query", "optimization", "indexes"],
        ),
        p(
            "Data Pipeline Design",
            "Design a robust, production-grade data pipeline",
            "You are a data engineering specialist. Design a robust data pipeline for the following requirements.\n\nYour design must cover:\n\n1. SOURCE ANALYSIS\n   - Data sources, formats, and volumes\n   - Schema detection and validation\n   - Change data capture strategy\n\n2. INGESTION LAYER\n   - Batch vs streaming decision with justification\n   - Extraction method and scheduling\n   - Error handling and dead letter queues\n   - Idempotency guarantees\n\n3. TRANSFORMATION LAYER\n   - Data quality checks and validation rules\n   - Transformation logic (cleaning, enrichment, aggregation)\n   - Schema evolution strategy\n   - Testing approach for transformations\n\n4. STORAGE LAYER\n   - Storage format selection (Parquet, Delta, Iceberg)\n   - Partitioning and clustering strategy\n   - Retention and archival policies\n   - Query patterns and optimization\n\n5. ORCHESTRATION\n   - DAG design and dependencies\n   - Retry and failure handling\n   - Monitoring, alerting, and SLAs\n   - Backfill and replay capabilities\n\n6. OPERATIONAL CONCERNS\n   - Cost estimation\n   - Security and access control\n   - Documentation and runbook\n\n{{input}}",
            "Data",
            &["data pipeline", "etl", "data engineering", "ingestion", "orchestration"],
        ),

        // ── Business (NEW category) ────────────────────────────────────────
        p(
            "Business Case",
            "Build a compelling, structured business case with financial analysis",
            "You are a management consultant. Build a compelling business case for the following proposal.\n\nStructure your business case as follows:\n\n1. EXECUTIVE SUMMARY (2-3 paragraphs)\n   - What we're proposing and why now\n   - Expected outcome and ROI\n   - Investment required\n\n2. PROBLEM STATEMENT\n   - Current state and pain points (quantified)\n   - Cost of inaction (financial, operational, strategic)\n   - Root cause analysis\n\n3. PROPOSED SOLUTION\n   - What we'll do (specific, actionable)\n   - Timeline and milestones\n   - Resource requirements (people, budget, tools)\n\n4. FINANCIAL ANALYSIS\n   - Total cost of ownership (3-year view)\n   - Expected benefits (quantified where possible)\n   - ROI calculation and payback period\n   - Sensitivity analysis (best/expected/worst case)\n\n5. RISK ASSESSMENT\n   - Top 5 risks with probability and impact\n   - Mitigation strategies for each\n   - Go/no-go criteria\n\n6. RECOMMENDATION\n   - Clear decision request\n   - Next steps with owners and dates\n\n{{input}}",
            "Business",
            &["business case", "roi", "proposal", "financial analysis", "strategy"],
        ),
        p(
            "Meeting Notes to Actions",
            "Transform raw meeting notes into structured, actionable summaries",
            "You are an executive assistant processing meeting notes. Transform the following raw meeting notes into a structured, actionable summary.\n\nOutput format:\n\nMEETING SUMMARY\n- Date: [extract or note 'not specified']\n- Attendees: [extract names mentioned]\n- Purpose: [1 sentence]\n\nKEY DECISIONS\n1. [Decision] \u{2014} [context/rationale]\n\nACTION ITEMS\n| # | Action | Owner | Due Date | Priority |\n|---|--------|-------|----------|----------|\n| 1 | ...    | ...   | ...      | High/Med/Low |\n\nOPEN QUESTIONS\n- [Questions raised but not resolved]\n\nPARKING LOT\n- [Topics deferred for future discussion]\n\nFOLLOW-UP\n- Next meeting: [date/topic if mentioned]\n- Dependencies: [what's blocking progress]\n\nRaw meeting notes:\n\n{{input}}",
            "Business",
            &["meeting", "notes", "action items", "summary", "productivity"],
        ),

        // ── DevOps (NEW category) ──────────────────────────────────────────
        p(
            "Incident Postmortem",
            "Write a thorough, blameless incident postmortem report",
            "You are a site reliability engineer writing a blameless postmortem. Create a thorough incident report from the following details.\n\nPOSTMORTEM TEMPLATE:\n\n## Incident Summary\n- Severity: [P1-P4 based on impact]\n- Duration: [start to full resolution]\n- Impact: [users/services affected, business impact]\n- Detection: [how was it found \u{2014} monitoring, customer report, etc.]\n\n## Timeline (UTC)\n| Time | Event |\n|------|-------|\n| HH:MM | First alert / detection |\n| HH:MM | Investigation began |\n| HH:MM | Root cause identified |\n| HH:MM | Mitigation applied |\n| HH:MM | Full resolution |\n\n## Root Cause\n[Detailed technical explanation of what went wrong and why]\n\n## Contributing Factors\n1. [Factor] \u{2014} [how it contributed]\n\n## What Went Well\n1. [Detection, response, communication, etc.]\n\n## What Could Be Improved\n1. [Area] \u{2014} [specific improvement]\n\n## Action Items\n| # | Action | Owner | Priority | Due Date |\n|---|--------|-------|----------|----------|\n\n## Lessons Learned\n[Key takeaways for the organization]\n\nIncident details:\n\n{{input}}",
            "DevOps",
            &["postmortem", "incident", "sre", "reliability", "blameless"],
        ),
        p(
            "Infrastructure as Code",
            "Generate production-grade Infrastructure as Code with security and docs",
            "You are a cloud infrastructure specialist. Generate production-grade Infrastructure as Code for the following requirements.\n\nYour IaC must follow these principles:\n- Idempotent and declarative\n- Parameterized (no hardcoded values)\n- Modular and reusable\n- Secure by default (least privilege, encryption at rest/transit)\n- Tagged for cost allocation and ownership\n- Documented with inline comments\n\nInclude:\n1. The IaC code (Terraform, CloudFormation, or Pulumi as appropriate)\n2. Variables/parameters file with sensible defaults\n3. Security considerations and IAM policies\n4. Networking configuration (VPC, subnets, security groups)\n5. Monitoring and alerting setup\n6. Cost estimate and optimization tips\n7. README with deployment instructions\n\nRequirements:\n\n{{input}}",
            "DevOps",
            &["infrastructure", "terraform", "iac", "cloud", "devops"],
        ),

        // ── Communication (NEW category) ───────────────────────────────────
        p(
            "Explain Like I'm 5",
            "Explain any concept in simple terms anyone can understand",
            "You are a gifted teacher who can explain ANY concept in simple terms that anyone can understand.\n\nRules:\n- Use everyday analogies and comparisons\n- No jargon \u{2014} if you must use a technical term, define it immediately\n- Use concrete examples, not abstract descriptions\n- Build from what the person already knows\n- Use 'imagine...' and 'think of it like...' framing\n- Keep sentences short and clear\n- Use numbered steps for processes\n- End with a one-sentence summary that captures the essence\n\nExplain this concept:\n\n{{input}}",
            "Communication",
            &["explain", "simple", "eli5", "teaching", "beginner"],
        ),
        p(
            "Presentation Outline",
            "Create a compelling, slide-by-slide presentation outline with speaker notes",
            "You are a presentation coach who has helped hundreds of speakers create compelling talks. Create a presentation outline for the following topic.\n\nPRESENTATION STRUCTURE:\n\nTITLE: [Compelling title \u{2014} benefit-focused, not topic-focused]\nDURATION: [Estimate based on content]\nAUDIENCE: [Who they are, what they know, what they need]\n\nSLIDE-BY-SLIDE OUTLINE:\n\nSlide 1 \u{2014} OPENING HOOK\n- [Surprising stat, provocative question, or relatable story]\n- Goal: grab attention in 30 seconds\n\nSlides 2-3 \u{2014} PROBLEM\n- [Establish the pain point the audience feels]\n- [Show the cost of the status quo]\n\nSlides 4-7 \u{2014} SOLUTION\n- [Your main argument/framework in 3-4 key points]\n- [Each point: claim \u{2192} evidence \u{2192} example]\n\nSlide 8 \u{2014} OBJECTION HANDLING\n- [Anticipate and address the top objection]\n\nSlide 9 \u{2014} CALL TO ACTION\n- [One specific thing the audience should do Monday morning]\n\nSlide 10 \u{2014} Q&A / CLOSE\n- [Memorable closing line that ties back to the opening]\n\nSPEAKER NOTES:\n- Key transitions between sections\n- Stories or examples to include\n- Questions to ask the audience\n- Timing markers\n\nTopic:\n\n{{input}}",
            "Communication",
            &["presentation", "slides", "public speaking", "outline", "talk"],
        ),

        // ── Learning (2) ──────────────────────────────────────────────────
        p(
            "Teach Me",
            "Get a personalized lesson on any topic from scratch",
            "You are a world-class teacher creating a personalized learning experience. Teach me the following topic from scratch.\n\nTeaching approach:\n1. START WITH WHY — Why does this matter? What can I do after learning this?\n2. PREREQUISITES CHECK — What should I already know? (brief bullet list)\n3. CORE CONCEPT — The fundamental mental model in 3-5 sentences\n4. BUILDING BLOCKS — Break it into 4-6 key concepts, each with:\n   - Clear explanation in plain language\n   - Concrete example that makes it click\n   - Common misconception to avoid\n5. HANDS-ON EXERCISE — A practical exercise I can try right now\n6. KNOWLEDGE CHECK — 5 questions to test my understanding (with answers)\n7. NEXT STEPS — What to learn next and recommended resources\n\nAdapt your explanation depth to the complexity of the topic. Use analogies freely.\n\nTeach me:\n\n{{input}}",
            "Learning",
            &["teach", "learn", "education", "tutorial"],
        ),
        p(
            "Study Guide",
            "Create a comprehensive study guide for any topic",
            "You are an expert educator. Create a comprehensive study guide for the following topic that would help someone pass an exam or deeply understand the subject.\n\nSTUDY GUIDE FORMAT:\n\n## Overview\n[What this topic covers and why it matters — 2-3 sentences]\n\n## Key Concepts (with difficulty ratings)\nFor each concept:\n- **Name** [Difficulty: Basic/Intermediate/Advanced]\n- Definition in one clear sentence\n- Example that illustrates it\n- How it connects to other concepts\n\n## Important Formulas / Rules / Patterns\n[List with explanations of when to use each]\n\n## Common Exam Questions & Answers\n1. [Question]\n   Answer: [Detailed answer with reasoning]\n\n## Mnemonics & Memory Aids\n[Tricks to remember key facts]\n\n## Practice Problems\n[5 problems of increasing difficulty with solutions]\n\n## Quick Reference Card\n[One-page cheat sheet of the most critical information]\n\nTopic:\n\n{{input}}",
            "Learning",
            &["study", "exam", "guide", "review", "education"],
        ),

        // ── Marketing (2) ─────────────────────────────────────────────────
        p(
            "Landing Page Copy",
            "Write conversion-focused landing page copy",
            "You are a conversion-focused copywriter. Write compelling landing page copy for the following product or service.\n\nFollow the proven landing page structure:\n\n1. HERO SECTION\n   - Headline: Clear value proposition (what + for whom + outcome)\n   - Subheadline: How it works in one sentence\n   - CTA button text: Action-oriented, specific\n\n2. PAIN POINTS (3)\n   - Problem the audience faces\n   - Emotional impact of the problem\n   - Why existing solutions fall short\n\n3. SOLUTION\n   - How the product solves each pain point\n   - Key differentiators (why this, not alternatives)\n\n4. FEATURES → BENEFITS (top 4-6)\n   For each: Feature → What it means → Why it matters to the user\n\n5. SOCIAL PROOF\n   - Suggested testimonial angles\n   - Stats/metrics to highlight\n   - Trust badges/logos to include\n\n6. OBJECTION HANDLING\n   - Top 3 objections and counters\n   - FAQ section (5 questions)\n\n7. FINAL CTA\n   - Urgency or scarcity element\n   - Risk reversal (guarantee, free trial)\n   - CTA button text\n\nWriting rules:\n- Write at 8th-grade reading level\n- Use 'you/your' (reader-focused, not product-focused)\n- Short sentences, short paragraphs\n- Power words: free, new, proven, guaranteed, instant\n\nProduct/service:\n\n{{input}}",
            "Marketing",
            &["landing page", "copywriting", "conversion", "sales"],
        ),
        p(
            "Content Calendar",
            "Create a detailed 4-week content calendar",
            "You are a content strategist. Create a detailed content calendar for the following brand or topic.\n\nCONTENT CALENDAR (4 weeks):\n\nFor each piece of content provide:\n- Day and platform (Blog, Twitter/X, LinkedIn, Newsletter, YouTube)\n- Content type (article, thread, carousel, video, poll)\n- Title/hook\n- Key message (1 sentence)\n- CTA (what action should the reader take?)\n- Hashtags or keywords\n\nWEEK 1: [Theme]\n| Day | Platform | Type | Title | Key Message |\n\nWEEK 2: [Theme]\n...\n\nWEEK 3: [Theme]\n...\n\nWEEK 4: [Theme]\n...\n\nCONTENT PILLARS\n1. [Pillar] — [% of content] — [goal]\n2. ...\n\nREPURPOSING STRATEGY\n- How to turn 1 blog post into 5+ pieces of content\n\nMETRICS TO TRACK\n- [Metric] — [target] — [tool]\n\nBrand/topic:\n\n{{input}}",
            "Marketing",
            &["content", "calendar", "social media", "strategy"],
        ),

        // ── Legal (1) ─────────────────────────────────────────────────────
        p(
            "Contract Review",
            "Analyze a contract for risks, red flags, and missing clauses",
            "You are a legal analyst. Review the following contract or agreement and provide a thorough analysis.\n\nIMPORTANT: This is for informational purposes only and does not constitute legal advice. Always consult a qualified attorney for legal decisions.\n\nYour analysis should cover:\n\n1. SUMMARY\n   - Parties involved\n   - Purpose and scope\n   - Key dates and duration\n   - Financial terms\n\n2. KEY OBLIGATIONS\n   - What each party must do\n   - Performance standards and SLAs\n   - Reporting requirements\n\n3. RISK ANALYSIS\n   | Risk | Severity | Clause Reference | Concern |\n   \n4. RED FLAGS\n   - Unusual or one-sided clauses\n   - Missing standard protections\n   - Ambiguous language that could be exploited\n   - Unlimited liability exposure\n\n5. MISSING CLAUSES\n   - Standard clauses that should be present but aren't\n   - Protections you should negotiate for\n\n6. NEGOTIATION RECOMMENDATIONS\n   - Top 5 changes to request, in priority order\n   - Suggested language for each change\n   - What to accept vs. what's a dealbreaker\n\nContract text:\n\n{{input}}",
            "Legal",
            &["contract", "legal", "review", "negotiation"],
        ),

        // ── Product (2) ───────────────────────────────────────────────────
        p(
            "PRD (Product Requirements)",
            "Write a complete Product Requirements Document",
            "You are a senior product manager. Write a complete Product Requirements Document for the following feature or product.\n\n## Product Requirements Document\n\n### 1. Overview\n- **Product/Feature Name:**\n- **Author:**\n- **Date:**\n- **Status:** Draft\n\n### 2. Problem Statement\n- What problem does this solve?\n- Who experiences this problem?\n- How do they solve it today?\n- Why is the current solution inadequate?\n\n### 3. Goals & Success Metrics\n| Goal | Metric | Target | Measurement Method |\n\n### 4. User Stories\nAs a [persona], I want to [action], so that [outcome].\nAcceptance criteria for each story.\n\n### 5. Functional Requirements\n| ID | Requirement | Priority (P0-P3) | Notes |\n\n### 6. Non-Functional Requirements\n- Performance targets\n- Security requirements\n- Accessibility requirements\n- Scalability expectations\n\n### 7. User Flow\nStep-by-step flow for the primary use case.\n\n### 8. Out of Scope\nWhat we are explicitly NOT doing in this version.\n\n### 9. Open Questions\nDecisions that need to be made.\n\n### 10. Timeline\n| Phase | Scope | Duration | Dependencies |\n\nFeature/product description:\n\n{{input}}",
            "Product",
            &["prd", "product", "requirements", "specification"],
        ),
        p(
            "User Research Questions",
            "Create a user research interview guide",
            "You are a UX researcher. Create a comprehensive user research interview guide for the following topic.\n\nINTERVIEW GUIDE\n\n## Research Objectives\n1. [What we want to learn]\n\n## Screening Criteria\n- Who qualifies for this study\n- Who to exclude and why\n\n## Warm-Up Questions (2-3 min)\n[Easy, open-ended questions to build rapport]\n\n## Core Questions (20-30 min)\nFor each question:\n- The question (open-ended, non-leading)\n- Why we're asking (internal note)\n- Follow-up probes if they give a short answer\n\n### Topic Area 1: Current Behavior\n[Questions about what they do today]\n\n### Topic Area 2: Pain Points\n[Questions about frustrations and unmet needs]\n\n### Topic Area 3: Reactions to Concept\n[Questions about the proposed solution]\n\n### Topic Area 4: Priorities\n[What matters most, trade-off questions]\n\n## Wrap-Up Questions (5 min)\n- Is there anything I didn't ask that you think is important?\n- Would you be open to a follow-up session?\n\n## ANALYSIS FRAMEWORK\n- Key themes to look for\n- How to synthesize across participants\n- Deliverable format\n\nResearch topic:\n\n{{input}}",
            "Product",
            &["user research", "interview", "ux", "discovery"],
        ),

        // ── Rust (2) ──────────────────────────────────────────────────────
        p(
            "Rust Code Review",
            "Perform a Rust-specific code review with ownership and safety checks",
            "You are a Rust expert performing a thorough code review. Analyze the following Rust code with attention to Rust-specific concerns.\n\nCheck for:\n\n1. OWNERSHIP & BORROWING\n   - Unnecessary clones or copies\n   - Lifetime issues or overly complex lifetime annotations\n   - Places where borrowing could replace ownership\n   - Missing or unnecessary Arc/Rc usage\n\n2. ERROR HANDLING\n   - Proper use of Result/Option\n   - Meaningful error types (thiserror/anyhow)\n   - No unwrap() in library code\n   - Error context propagation with .context()\n\n3. IDIOMATIC RUST\n   - Iterator chains vs manual loops\n   - Pattern matching completeness\n   - Builder pattern where appropriate\n   - Newtype pattern for type safety\n   - Proper use of traits and generics\n\n4. PERFORMANCE\n   - Unnecessary allocations (String vs &str, Vec vs slice)\n   - Missing #[inline] on hot paths\n   - Proper use of Cow<str> for flexible ownership\n   - Async considerations (Send + Sync bounds)\n\n5. SAFETY\n   - No unsafe without justification\n   - Proper Send/Sync implementations\n   - Integer overflow potential\n   - Proper handling of FFI boundaries\n\n6. CARGO & ECOSYSTEM\n   - Appropriate dependency choices\n   - Feature flag usage\n   - Proper edition usage\n\nFor each issue, provide the fix as idiomatic Rust code.\n\n{{input}}",
            "Rust",
            &["rust", "code review", "ownership", "safety"],
        ),
        p(
            "Rust From Scratch",
            "Implement production-quality idiomatic Rust code from a description",
            "You are a Rust expert writing idiomatic, production-quality Rust code. Implement the following from scratch.\n\nYour Rust code must:\n- Compile on stable Rust (latest edition)\n- Use proper error handling (Result<T, E> with thiserror or anyhow)\n- Follow Rust API guidelines (https://rust-lang.github.io/api-guidelines/)\n- Use Iterator and closure patterns idiomatically\n- Prefer &str over String in function parameters\n- Use Cow<str> when ownership is conditional\n- Derive Debug, Clone, and other standard traits appropriately\n- Add doc comments (///) on all public items\n- Include unit tests in a #[cfg(test)] module\n- No unwrap() except in tests\n- Use proper module organization\n\nAfter the code, explain:\n1. Key design decisions and why\n2. Ownership/borrowing strategy chosen\n3. Error handling approach\n4. How to extend it\n\n{{input}}",
            "Rust",
            &["rust", "implement", "idiomatic", "production"],
        ),

        // ── Personal (1) ──────────────────────────────────────────────────
        p(
            "Decision Framework",
            "Make a well-reasoned decision using a structured framework",
            "You are a strategic thinking coach. Help me make a well-reasoned decision using a structured framework.\n\nDECISION ANALYSIS:\n\n## 1. Clarify the Decision\n- What exactly am I deciding?\n- What are my constraints (time, money, energy)?\n- What's the decision deadline?\n- Is this reversible or irreversible?\n\n## 2. Options (generate at least 4)\nFor each option:\n- Description\n- Pros (be specific)\n- Cons (be honest)\n- What would need to be true for this to be the best choice?\n\n## 3. Criteria Weighting\n| Criteria | Weight (1-10) |\nRate each option against each criterion.\n\n## 4. Second-Order Effects\n- If I choose X, what happens 6 months later? 2 years later?\n- Who else is affected and how?\n- What doors does this open or close?\n\n## 5. Pre-Mortem\n- Imagine it's 1 year from now and this decision was a disaster. What went wrong?\n- How can I mitigate those risks now?\n\n## 6. Recommendation\n- Best option with reasoning\n- Implementation steps\n- Review point to reassess\n\nDecision I need to make:\n\n{{input}}",
            "Personal",
            &["decision", "framework", "strategy", "analysis"],
        ),

        // ── UI/UX (8) ─────────────────────────────────────────────────────
        p(
            "Apple Design System",
            "Design or review an interface using Apple's Human Interface Guidelines",
            "You are a senior designer deeply versed in Apple's Human Interface Guidelines (HIG). Design or review the following interface using Apple's design principles.\n\nApply these Apple design principles:\n1. CLARITY — Content is the focus. Use San Francisco font family, generous whitespace, system colors. Avoid unnecessary decoration.\n2. DEFERENCE — Fluid motion, crisp graphics. The UI helps users understand content without competing with it.\n3. DEPTH — Visual layers and realistic motion create hierarchy. Use translucency, shadows, and parallax.\n4. CONSISTENCY — Familiar controls, standard gestures, predictable behavior. Follow SF Symbols for iconography.\n\nPlatform-specific guidelines:\n- iOS: Bottom tab bar navigation, large titles, haptic feedback, swipe gestures, safe area insets\n- macOS: Menu bar, toolbar, sidebar navigation, keyboard shortcuts, hover states, window resizing\n- Both: Dynamic Type, Dark Mode support, accessibility (VoiceOver), SF Symbols, system colors\n\nDesign deliverables:\n- Component hierarchy and layout description\n- Color usage (system colors, semantic colors)\n- Typography (SF Pro, SF Mono, size scale)\n- Spacing and sizing (8pt grid system)\n- Interaction patterns and animations\n- Accessibility considerations\n- Dark/Light mode variants\n\n{{input}}",
            "UI/UX",
            &["apple", "hig", "ios", "macos", "design system"],
        ),
        p(
            "Material Design (Google)",
            "Design or review an interface using Google's Material Design 3",
            "You are a designer expert in Google's Material Design 3 (Material You). Design or review the following interface using Material Design principles.\n\nApply Material Design 3 principles:\n1. PERSONAL — Dynamic color from user's wallpaper, expressive yet functional\n2. ADAPTIVE — Responsive layouts across phone, tablet, desktop, foldable\n3. UNIFIED — Consistent cross-platform experience with platform-appropriate patterns\n\nKey Material 3 components and patterns:\n- Navigation: Bottom bar (3-5 items), Navigation rail (tablets), Navigation drawer (desktop)\n- Surfaces: Cards, sheets, dialogs with elevation and tonal color\n- Typography: Roboto/Google Sans, 5 roles (Display, Headline, Title, Body, Label) with 3 sizes each\n- Color: Primary, secondary, tertiary, error + surface, outline, inverse roles\n- Shape: Rounded corners (4-28dp scale), container shapes\n- Motion: Emphasized easing, 300ms standard duration, shared element transitions\n- Icons: Material Symbols (outlined, rounded, or sharp)\n\nYour design must include:\n- Layout grid (columns, margins, gutters) for target screen sizes\n- Color scheme using Material 3 token system\n- Component selection with proper states (enabled, hovered, focused, pressed, disabled)\n- Elevation hierarchy (0dp to 5dp levels)\n- Responsive breakpoints (compact, medium, expanded)\n- Touch targets (minimum 48dp)\n- Dark theme variant\n\n{{input}}",
            "UI/UX",
            &["material design", "google", "android", "material you"],
        ),
        p(
            "Minimal Scandinavian UI",
            "Create a clean, functional interface following Nordic design principles",
            "You are a designer specializing in Scandinavian minimalist design. Create a clean, functional interface following Nordic design principles.\n\nScandinavian design principles:\n1. SIMPLICITY — Remove everything that isn't essential. Every element must earn its place.\n2. FUNCTIONALITY — Form follows function. Beauty comes from purpose, not decoration.\n3. LIGHT & SPACE — Generous whitespace, light backgrounds, natural breathing room.\n4. NATURAL MATERIALS — Warm neutrals, muted earth tones, organic shapes.\n5. DEMOCRATIC DESIGN — Accessible to everyone, intuitive without instruction.\n\nDesign specifications:\n- Color palette: White/off-white base (#FAFAFA), warm grays, one accent color (muted), black for text\n- Typography: Clean sans-serif (Inter, Instrument Sans, or similar), large body text (16-18px), generous line height (1.6-1.8)\n- Spacing: 8px base unit, generous margins (32-64px), visible breathing room between sections\n- Components: Borderless inputs, subtle hover states, understated buttons, no gradients\n- Imagery: High-quality photography, desaturated, natural subjects\n- Animation: Subtle, purposeful, 200-300ms, ease-out timing\n- Layout: Single-column where possible, maximum content width (680-720px for text)\n- No: Shadows heavier than 2px, bright colors, decorative elements, complex animations, rounded corners > 8px\n\n{{input}}",
            "UI/UX",
            &["scandinavian", "minimal", "nordic", "clean"],
        ),
        p(
            "Dashboard Design",
            "Design an effective, actionable data dashboard",
            "You are a data visualization and dashboard design expert. Design an effective, actionable dashboard for the following requirements.\n\nDashboard design framework:\n\n1. INFORMATION HIERARCHY\n   - KPIs first: 3-5 key metrics prominently displayed at the top\n   - Supporting charts: arranged by importance, left-to-right, top-to-bottom\n   - Detail tables: expandable, below the fold\n   - Filters: sidebar or top bar, always visible\n\n2. CHART SELECTION\n   - Trends over time \u{2192} Line chart\n   - Part-to-whole \u{2192} Donut/pie (max 6 segments) or stacked bar\n   - Comparison \u{2192} Horizontal bar chart\n   - Distribution \u{2192} Histogram or box plot\n   - Correlation \u{2192} Scatter plot\n   - Geographic \u{2192} Map with heat overlay\n   - Single KPI \u{2192} Number with sparkline and trend indicator\n\n3. VISUAL DESIGN\n   - Grid: 12-column, cards with subtle borders\n   - Colors: Sequential palette for magnitude, categorical for groups (max 6-8 colors)\n   - Typography: Tabular numerals, monospace for numbers, clear hierarchy\n   - White space: Minimum 16px between cards\n   - Responsive: Collapse to stacked layout on mobile\n\n4. INTERACTIVITY\n   - Cross-filtering: clicking one chart filters others\n   - Drill-down: click KPI to see breakdown\n   - Time range selector: presets (Today, 7d, 30d, 90d, YTD, Custom)\n   - Export: CSV, PDF, share link\n   - Tooltips: on hover, showing exact values\n\n5. ACCESSIBILITY\n   - Color-blind safe palette\n   - Screen reader labels for all charts\n   - Keyboard navigation for filters and controls\n\nProvide: layout wireframe description, chart specifications, color palette, component list, and interaction flows.\n\n{{input}}",
            "UI/UX",
            &["dashboard", "data visualization", "charts", "analytics"],
        ),
        p(
            "Mobile App Design",
            "Design a mobile application with a focus on usability and delight",
            "You are a senior mobile app designer. Design the following mobile application with a focus on usability and delight.\n\nMobile design principles:\n1. THUMB-FRIENDLY — Primary actions in the bottom 60% of screen. Navigation at bottom.\n2. PROGRESSIVE DISCLOSURE — Show only what's needed. Reveal complexity on demand.\n3. FEEDBACK — Every tap gets a response (haptic, visual, audio). Loading states for all async operations.\n4. OFFLINE-FIRST — Design for intermittent connectivity. Show cached data, queue actions.\n\nYour design must include:\n\nNAVIGATION STRUCTURE\n- Primary navigation (bottom tab bar: max 5 items)\n- Secondary navigation (within each tab)\n- Screen flow diagram\n\nSCREEN DESIGNS (for each key screen)\n- Layout description (component placement, spacing)\n- Component states (default, loading, empty, error, success)\n- Gestures (swipe, long-press, pull-to-refresh)\n- Transitions between screens\n\nDESIGN TOKENS\n- Colors: primary, secondary, background, surface, error, text hierarchy\n- Typography: scale from 12-34sp, weights\n- Spacing: 4/8/12/16/24/32/48px scale\n- Border radius: 4/8/12/16/24px\n- Elevation: 3 levels (low, medium, high)\n\nPATTERNS\n- Lists (with pull-to-refresh, infinite scroll, empty state)\n- Forms (with validation, keyboard type per field)\n- Modals and sheets (bottom sheet preferred over modal)\n- Search (with recent, suggestions, filters)\n- Onboarding (max 3 screens)\n\nPLATFORM CONSIDERATIONS\n- iOS: Safe area, notch, Dynamic Island, SF Symbols\n- Android: Material 3, edge-to-edge, predictive back\n\n{{input}}",
            "UI/UX",
            &["mobile", "app design", "ios", "android", "usability"],
        ),
        p(
            "Landing Page Design",
            "Design a high-converting landing page for a product or service",
            "You are a conversion-focused web designer. Design a high-converting landing page for the following product or service.\n\nLanding page design framework:\n\nABOVE THE FOLD (first viewport)\n- Hero: Large headline (benefit-focused, 4-8 words), subheadline (1 sentence), primary CTA button\n- Visual: Product screenshot, illustration, or hero image (not stock photos)\n- Social proof: Logo bar or trust badge strip\n- No navigation menu (single CTA focus)\n\nSECTIONS (in order)\n1. Problem statement \u{2014} 3 pain points with icons\n2. Solution \u{2014} How the product solves each pain point\n3. Features \u{2192} Benefits \u{2014} 3-6 features, each with benefit statement\n4. Social proof \u{2014} Testimonials (photo, name, role, quote), case study metrics\n5. How it works \u{2014} 3-step visual process\n6. Pricing (if applicable) \u{2014} Max 3 tiers, highlight recommended\n7. FAQ \u{2014} 5-7 questions in accordion\n8. Final CTA \u{2014} Restate the value proposition, urgency element\n\nDESIGN SPECIFICATIONS\n- Max width: 1200px content area\n- Typography: Large headlines (48-72px), readable body (18-20px), 1.6 line height\n- CTA buttons: High contrast, rounded, minimum 48px height, action verb text\n- Whitespace: 80-120px between sections\n- Color: One dominant brand color, one accent, neutral backgrounds\n- Mobile: All sections stack vertically, CTA stays visible (sticky or repeated)\n- Speed: Design for fast load (no heavy animations, optimized images, lazy load below fold)\n\nCONVERSION ELEMENTS\n- Above-fold CTA visible without scrolling\n- Social proof within 2 scrolls of CTA\n- Exit-intent elements (if appropriate)\n- Form fields minimized (name + email only for signups)\n- Trust signals near every CTA (guarantee, security badges)\n\n{{input}}",
            "UI/UX",
            &["landing page", "conversion", "web design", "cta"],
        ),
        p(
            "Design Tokens",
            "Create a comprehensive design token system for a project",
            "You are a design systems engineer. Create a comprehensive design token system for the following project.\n\nDesign tokens are the atomic design decisions that form the foundation of a design system. Define all tokens for this project.\n\nTOKEN CATEGORIES:\n\n1. COLOR\n   - Primitives: Full color scales (50-950 for each hue)\n   - Semantic: primary, secondary, tertiary, error, warning, success, info\n   - Surface: background, surface, on-surface, outline, divider\n   - Interactive: hover, pressed, focused, disabled states\n   - Dark mode: All tokens must have dark mode variants\n   - Format: CSS custom properties AND JSON for tooling\n\n2. TYPOGRAPHY\n   - Font families: primary (sans), secondary (serif if needed), mono\n   - Size scale: xs(12), sm(14), base(16), lg(18), xl(20), 2xl(24), 3xl(30), 4xl(36), 5xl(48)\n   - Weight scale: light(300), regular(400), medium(500), semibold(600), bold(700)\n   - Line height: tight(1.25), normal(1.5), relaxed(1.75)\n   - Letter spacing: tight(-0.025em), normal(0), wide(0.025em)\n\n3. SPACING\n   - Scale: 0, 1(4px), 2(8px), 3(12px), 4(16px), 5(20px), 6(24px), 8(32px), 10(40px), 12(48px), 16(64px), 20(80px), 24(96px)\n   - Named: page-margin, section-gap, card-padding, input-padding, button-padding\n\n4. SIZING\n   - Icon sizes: sm(16), md(20), lg(24), xl(32)\n   - Avatar sizes: xs(24), sm(32), md(40), lg(48), xl(64)\n   - Touch targets: minimum 44px\n\n5. BORDER\n   - Width: thin(1px), medium(2px), thick(4px)\n   - Radius: none(0), sm(4px), md(8px), lg(12px), xl(16px), full(9999px)\n\n6. ELEVATION / SHADOW\n   - Level 1: 0 1px 2px rgba(0,0,0,0.05)\n   - Level 2: 0 4px 6px rgba(0,0,0,0.07)\n   - Level 3: 0 10px 15px rgba(0,0,0,0.1)\n\n7. MOTION\n   - Duration: fast(100ms), normal(200ms), slow(300ms), deliberate(500ms)\n   - Easing: ease-out, ease-in-out, spring\n   - Transitions: fade, slide, scale, collapse\n\nOutput format: Provide tokens in both CSS custom properties and JSON format.\n\n{{input}}",
            "UI/UX",
            &["design tokens", "design system", "css", "theming", "variables"],
        ),
        p(
            "Figma Component Spec",
            "Write a detailed component specification for design-to-development handoff",
            "You are a design engineer who bridges design and development. Write a detailed component specification that a developer can implement exactly from your description.\n\nCOMPONENT SPECIFICATION:\n\n1. OVERVIEW\n   - Component name and purpose\n   - When to use (and when NOT to use)\n   - Related components\n\n2. ANATOMY\n   - Visual breakdown of all sub-elements\n   - Required vs optional elements\n   - Slot/content areas\n\n3. VARIANTS\n   - Size: sm, md, lg (with exact dimensions)\n   - State: default, hover, active, focused, disabled, loading, error\n   - Style: filled, outlined, ghost/text\n   - For each variant: exact colors, spacing, typography, border\n\n4. LAYOUT & SPACING\n   - Internal padding (per size variant)\n   - Gap between sub-elements\n   - Minimum/maximum width constraints\n   - Alignment rules\n\n5. INTERACTION\n   - Click/tap behavior\n   - Hover state transition (duration, easing)\n   - Focus ring style (for keyboard navigation)\n   - Loading state behavior\n   - Error state behavior\n\n6. RESPONSIVE BEHAVIOR\n   - How the component adapts at different breakpoints\n   - Touch vs pointer adaptations\n   - Minimum touch target size (44px)\n\n7. ACCESSIBILITY\n   - ARIA role and attributes\n   - Keyboard interaction pattern\n   - Screen reader announcement\n   - Focus management\n   - Color contrast requirements (4.5:1 for text)\n\n8. CODE REFERENCE\n   - Props/API surface\n   - Usage examples (3 common patterns)\n   - Edge cases to handle\n\n{{input}}",
            "UI/UX",
            &["component", "specification", "figma", "handoff", "design engineering"],
        ),

        // ── Testing (NEW category) ────────────────────────────────────────
        p(
            "Unit Test Suite",
            "Write a comprehensive unit test suite for any code",
            "You are a testing expert. Write a comprehensive unit test suite for the following code.\n\nFor EACH public function/method, write tests covering:\n\nHAPPY PATH\n- Normal inputs that should work correctly\n- Expected return values verified precisely\n\nEDGE CASES\n- Empty inputs (empty string, empty vec, zero, None)\n- Boundary values (0, 1, -1, MAX, MIN)\n- Maximum lengths and sizes\n- Unicode and special characters\n- Whitespace variations\n\nERROR CASES\n- Invalid inputs that should return errors\n- Null/None/nil handling\n- Type mismatches\n- Network/IO failures (if applicable)\n\nREGRESSION CASES\n- Bugs that were fixed (if mentioned)\n- Previously failing scenarios\n\nTest quality rules:\n- One assertion per test (focused, named tests)\n- Descriptive test names: test_<function>_<scenario>_<expected>\n- Arrange-Act-Assert pattern\n- No test interdependencies\n- Mock external dependencies\n- Tests should run in < 100ms each\n\nOutput: Complete test file with imports, helpers, and all test functions.\n\n{{input}}",
            "Testing",
            &["unit test", "test suite", "coverage", "assertions"],
        ),
        p(
            "Integration Test Plan",
            "Design a comprehensive integration test plan for a system or feature",
            "You are a QA architect. Design a comprehensive integration test plan for the following system or feature.\n\nTEST PLAN STRUCTURE:\n\n1. SCOPE\n   - What is being tested (components, interfaces, data flows)\n   - What is NOT being tested (out of scope)\n   - Dependencies and prerequisites\n\n2. TEST ENVIRONMENT\n   - Required services and their versions\n   - Database setup (schema, seed data)\n   - External service mocks/stubs\n   - Configuration requirements\n\n3. TEST SCENARIOS (for each integration point)\n   | ID | Scenario | Input | Expected Output | Priority |\n   |---|---------|-------|----------------|----------|\n   \n   Cover:\n   - Happy path end-to-end flows\n   - Error propagation across services\n   - Timeout and retry behavior\n   - Concurrent access patterns\n   - Data consistency across services\n   - Authentication and authorization flows\n   - Rate limiting behavior\n\n4. DATA REQUIREMENTS\n   - Test fixtures (specific data needed)\n   - Data setup and teardown procedures\n   - Shared vs isolated test data\n\n5. AUTOMATION STRATEGY\n   - Which tests to automate first (ROI-based)\n   - Framework and tool recommendations\n   - CI/CD integration approach\n   - Reporting and alerting\n\n6. RISK ASSESSMENT\n   - Most likely failure points\n   - Hardest-to-test scenarios\n   - Monitoring gaps\n\n{{input}}",
            "Testing",
            &["integration test", "test plan", "qa", "automation", "scenarios"],
        ),
        p(
            "E2E Test Script",
            "Write end-to-end test scripts for user flows",
            "You are a test automation engineer. Write end-to-end test scripts for the following user flows.\n\nFor each flow, write complete, runnable test code including:\n- Setup: User creation, data seeding, authentication\n- Steps: Each user action with assertions after each step\n- Teardown: Cleanup of created data\n- Screenshots/snapshots at key points\n- Retry logic for flaky interactions (network, animations)\n- Meaningful error messages on failure\n\nTest each flow for:\n- Happy path (everything works)\n- Validation errors (bad input)\n- Permission denied (unauthorized)\n- Network failure (offline/timeout)\n- Concurrent users (if applicable)\n\nBest practices:\n- Use page object pattern or similar abstraction\n- Wait for elements properly (no arbitrary sleeps)\n- Test IDs for reliable element selection\n- Independent tests (no order dependency)\n- Idempotent (can run multiple times)\n\n{{input}}",
            "Testing",
            &["e2e", "end-to-end", "automation", "user flows", "selenium"],
        ),
        p(
            "Load Test Plan",
            "Design a load testing strategy for a system",
            "You are a performance engineer. Design a load testing strategy for the following system.\n\nLOAD TEST PLAN:\n\n1. OBJECTIVES\n   - Performance targets (response time P50, P95, P99)\n   - Throughput targets (requests/second)\n   - Concurrent user capacity\n   - Resource utilization limits (CPU, memory, connections)\n\n2. TEST TYPES\n   - Smoke test: 1-5 users, verify baseline (5 min)\n   - Load test: Expected load, sustained (30 min)\n   - Stress test: 150-200% of expected load (15 min)\n   - Spike test: Sudden burst (10x normal for 2 min)\n   - Soak test: Normal load for extended period (4-8 hours)\n\n3. SCENARIOS\n   For each critical user flow:\n   - Virtual user script (step by step)\n   - Think time between actions (realistic)\n   - Data parameterization\n   - Assertion thresholds\n\n4. INFRASTRUCTURE\n   - Load generator sizing and location\n   - Monitoring setup (APM, metrics, logs)\n   - Alerting thresholds during test\n\n5. ANALYSIS\n   - Key metrics to watch\n   - Bottleneck identification approach\n   - How to determine pass/fail\n   - Report template\n\n{{input}}",
            "Testing",
            &["load test", "performance", "stress test", "benchmarking", "capacity"],
        ),

        // ── Business (additional) ─────────────────────────────────────────
        p(
            "Strategic Plan",
            "Develop a strategic plan for a business challenge",
            "You are a management consultant at a top-tier firm. Develop a strategic plan for the following business challenge.\n\nSTRATEGIC PLAN:\n\n1. SITUATION ANALYSIS\n   - Current state assessment\n   - Market landscape (competitors, trends, disruptions)\n   - SWOT analysis (Strengths, Weaknesses, Opportunities, Threats)\n   - Key stakeholder map\n\n2. STRATEGIC VISION\n   - 3-year vision statement\n   - Strategic objectives (3-5, SMART format)\n   - Key results for each objective (OKRs)\n\n3. STRATEGIC OPTIONS\n   - Option A: [Description] \u{2014} Risk/Reward analysis\n   - Option B: [Description] \u{2014} Risk/Reward analysis\n   - Option C: [Description] \u{2014} Risk/Reward analysis\n   - Recommended option with justification\n\n4. EXECUTION PLAN\n   - Phase 1 (0-6 months): Quick wins and foundation\n   - Phase 2 (6-12 months): Core initiatives\n   - Phase 3 (12-36 months): Scale and optimize\n   - Resource requirements per phase\n\n5. FINANCIAL MODEL\n   - Investment required (headcount, technology, marketing)\n   - Revenue projections (conservative, base, optimistic)\n   - Break-even analysis\n   - Key financial assumptions\n\n6. RISK MITIGATION\n   - Top 5 execution risks\n   - Mitigation strategy for each\n   - Decision gates and pivot criteria\n\n7. GOVERNANCE\n   - Steering committee structure\n   - Review cadence (monthly, quarterly)\n   - Success metrics and reporting\n\n{{input}}",
            "Business",
            &["strategy", "strategic plan", "consulting", "vision", "execution"],
        ),
        p(
            "Competitive Analysis",
            "Conduct a thorough competitive analysis for a product or market",
            "You are a market analyst. Conduct a thorough competitive analysis for the following product or market.\n\nCOMPETITIVE ANALYSIS:\n\n1. MARKET OVERVIEW\n   - Market size and growth rate\n   - Key market segments\n   - Industry trends and disruptions\n   - Regulatory considerations\n\n2. COMPETITOR PROFILES (for each major competitor)\n   | Dimension | Competitor A | Competitor B | You |\n   |-----------|-------------|-------------|-----|\n   | Positioning | | | |\n   | Target market | | | |\n   | Key features | | | |\n   | Pricing | | | |\n   | Revenue (est.) | | | |\n   | Team size (est.) | | | |\n   | Funding | | | |\n   | Strengths | | | |\n   | Weaknesses | | | |\n\n3. FEATURE COMPARISON MATRIX\n   | Feature | Us | Comp A | Comp B | Comp C |\n   (Rate: Yes/No/Partial, or 1-5 scale)\n\n4. DIFFERENTIATION ANALYSIS\n   - Our unique advantages\n   - Their unique advantages\n   - Parity features (table stakes)\n   - Gap analysis (what we're missing)\n\n5. STRATEGIC POSITIONING\n   - Positioning map (2x2 or perceptual map description)\n   - Recommended positioning statement\n   - Messaging differentiation\n\n6. OPPORTUNITIES\n   - Underserved segments\n   - Feature gaps to exploit\n   - Pricing opportunities\n   - Partnership possibilities\n\n{{input}}",
            "Business",
            &["competitive analysis", "market research", "competitors", "positioning"],
        ),
        p(
            "OKR Framework",
            "Create a comprehensive OKR framework for a team or initiative",
            "You are an OKR coach who has helped hundreds of teams set effective goals. Create a comprehensive OKR framework for the following team or initiative.\n\nOKR FRAMEWORK:\n\nFor each Objective:\n- The Objective should be qualitative, inspiring, time-bound, and achievable\n- Write 3-5 Key Results that are quantitative, measurable, and ambitious (70% target confidence)\n\nCOMPANY/TEAM LEVEL:\n\nObjective 1: [Inspiring statement]\n  KR1: [Metric] from [X] to [Y] by [date]\n  KR2: [Metric] from [X] to [Y] by [date]\n  KR3: [Metric] from [X] to [Y] by [date]\n\nObjective 2: ...\nObjective 3: ...\n\nFor each KR, provide:\n- Current baseline value\n- Target value\n- How to measure it (data source, frequency)\n- Leading indicators to track weekly\n- Initiatives/projects that drive this KR\n\nALIGNMENT MAP:\n- How team OKRs connect to company OKRs\n- Cross-team dependencies\n- Shared KRs between teams\n\nCADENCE:\n- Annual: Company objectives\n- Quarterly: Team OKRs (set, review, score)\n- Weekly: Confidence updates (red/yellow/green)\n- Bi-weekly: 1:1 check-ins on KR progress\n\nSCORING GUIDE:\n- 0.0-0.3: Failed to make progress\n- 0.4-0.6: Made progress but fell short\n- 0.7-0.9: Delivered (sweet spot for stretch goals)\n- 1.0: Achieved 100% (goal may have been too easy)\n\n{{input}}",
            "Business",
            &["okr", "goals", "objectives", "key results", "planning"],
        ),

        // ── DevOps (additional) ───────────────────────────────────────────
        p(
            "Kubernetes Deployment",
            "Design a production-ready Kubernetes deployment for an application",
            "You are a Kubernetes platform engineer. Design a production-ready Kubernetes deployment for the following application.\n\nDEPLOYMENT SPECIFICATION:\n\n1. ARCHITECTURE\n   - Namespace strategy\n   - Service mesh requirements (if any)\n   - Ingress/load balancer configuration\n   - Network policies\n\n2. WORKLOAD DEFINITIONS\n   - Deployment manifests (replicas, strategy, pod spec)\n   - Resource requests and limits (CPU, memory)\n   - Liveness and readiness probes\n   - Pod disruption budgets\n   - Horizontal pod autoscaler configuration\n\n3. CONFIGURATION\n   - ConfigMaps for non-sensitive config\n   - Secrets management (sealed secrets, external secrets, vault)\n   - Environment variable mapping\n\n4. STORAGE\n   - PersistentVolumeClaims (if needed)\n   - Storage class selection\n   - Backup strategy\n\n5. OBSERVABILITY\n   - Prometheus metrics and ServiceMonitor\n   - Grafana dashboard specifications\n   - Log aggregation (format, labels)\n   - Alerting rules (SLOs, error rates)\n\n6. SECURITY\n   - RBAC policies\n   - Pod security standards\n   - Network policies\n   - Image scanning and admission control\n   - Secret rotation\n\n7. CI/CD INTEGRATION\n   - Helm chart or Kustomize structure\n   - ArgoCD/Flux application manifest\n   - Rollout strategy (blue-green, canary)\n   - Rollback procedure\n\nProvide complete YAML manifests for all resources.\n\n{{input}}",
            "DevOps",
            &["kubernetes", "k8s", "deployment", "containers", "orchestration"],
        ),

        // ── Writing (additional) ──────────────────────────────────────────
        p(
            "Technical RFC",
            "Write a technical RFC to propose a significant technical change",
            "You are a senior engineer writing a technical RFC (Request for Comments) to propose a significant technical change.\n\nRFC TEMPLATE:\n\n# RFC: [Title]\n\n**Author:** [Name]\n**Status:** Draft\n**Created:** [Date]\n\n## Summary\nOne paragraph explaining the proposal.\n\n## Motivation\n- What problem does this solve?\n- Why is it important now?\n- What happens if we don't do this?\n\n## Detailed Design\n- Technical approach (architecture, components, data flow)\n- API changes (if any, with before/after examples)\n- Data model changes (migration plan)\n- Code examples showing the new pattern\n\n## Alternatives Considered\nFor each alternative:\n- Description\n- Pros and cons\n- Why it was not chosen\n\n## Migration Plan\n- Phased rollout steps\n- Backward compatibility approach\n- Rollback plan\n- Timeline estimate\n\n## Risks and Mitigations\n| Risk | Probability | Impact | Mitigation |\n\n## Open Questions\n- Decisions that need input from others\n\n## References\n- Related RFCs, design docs, or external resources\n\nWrite the complete RFC:\n\n{{input}}",
            "Writing",
            &["rfc", "technical writing", "proposal", "design doc", "architecture"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn builtin_prompts_not_empty() {
        let prompts = builtin_prompts();
        assert!(!prompts.is_empty());
        assert!(
            prompts.len() >= 109,
            "Expected at least 109 prompts, got {}",
            prompts.len()
        );
    }

    #[test]
    fn all_prompts_have_required_fields() {
        for p in builtin_prompts() {
            assert!(!p.name.is_empty(), "Prompt has empty name");
            assert!(
                !p.description.is_empty(),
                "Prompt '{}' has empty description",
                p.name
            );
            assert!(
                !p.template.is_empty(),
                "Prompt '{}' has empty template",
                p.name
            );
            assert!(
                !p.category.is_empty(),
                "Prompt '{}' has empty category",
                p.name
            );
            assert!(
                p.template.contains("{{input}}"),
                "Prompt '{}' missing {{{{input}}}} placeholder",
                p.name
            );
        }
    }

    #[test]
    fn no_duplicate_names() {
        let prompts = builtin_prompts();
        let mut names = HashSet::new();
        for p in &prompts {
            assert!(names.insert(&p.name), "Duplicate prompt name: {}", p.name);
        }
    }

    #[test]
    fn expected_categories_exist() {
        let prompts = builtin_prompts();
        let categories: HashSet<&str> = prompts.iter().map(|p| p.category.as_str()).collect();
        for expected in &[
            "Writing",
            "Coding",
            "Translation",
            "Analysis",
            "Creative",
            "Productivity",
            "Engineering",
            "Design",
            "Best Practices",
            "Git",
            "Data",
            "Business",
            "DevOps",
            "Communication",
            "Learning",
            "Marketing",
            "Legal",
            "Product",
            "Rust",
            "Personal",
            "UI/UX",
            "Testing",
        ] {
            assert!(
                categories.contains(expected),
                "Missing category: {expected}"
            );
        }
    }

    #[test]
    fn each_category_has_minimum_prompts() {
        let prompts = builtin_prompts();
        let mut counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for p in &prompts {
            *counts.entry(p.category.as_str()).or_default() += 1;
        }
        for (cat, count) in &counts {
            assert!(
                *count >= 1,
                "Category '{cat}' has only {count} prompts, expected at least 1"
            );
        }
    }
}
