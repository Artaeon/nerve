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
            prompts.len() >= 100,
            "Expected at least 100 prompts, got {}",
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
                *count >= 5,
                "Category '{cat}' has only {count} prompts, expected at least 5"
            );
        }
    }
}
