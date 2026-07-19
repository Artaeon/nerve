# Changelog

All notable changes to Nerve will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1](https://github.com/Artaeon/nerve/compare/nerve-v0.2.0...nerve-v0.2.1) (2026-07-19)


### Features

* **.nerve:** gate nerve's own jobs on clippy and fmt, not just check+test ([c27f246](https://github.com/Artaeon/nerve/commit/c27f246ec6a45811e7672af6f53583622ad4c718))
* /init engineering brief — one-shot repo analysis into .nerve/brief.md ([e1a2e5b](https://github.com/Artaeon/nerve/commit/e1a2e5b9bc96db5ff18d049cbc6bc9dde62f7c4a))
* /tokens and /context report the real sent payload, not just stored ([b52e677](https://github.com/Artaeon/nerve/commit/b52e67769e54f07f8c67bc2087be75850aea3936))
* **agent:** self-decompose loop — split a task into sub-tasks and execute each ([c908b4e](https://github.com/Artaeon/nerve/commit/c908b4ef6461c46a19aa64c26cc3f361a1c2d3a6))
* **agent:** subprocess isolation for decompose steps — kill the worker wedge ([1cda349](https://github.com/Artaeon/nerve/commit/1cda349cab85813a1f115d412e3c2fc3e9b393e4))
* auto design-check — surface design inconsistencies after UI edits ([69d0c41](https://github.com/Artaeon/nerve/commit/69d0c41fa7bc23e6fb4614828e8b27c65732e350))
* auto-capture per-turn activity log so context is saved by default ([e53e40f](https://github.com/Artaeon/nerve/commit/e53e40fd1e15de3c80a0a0de43176c4647d6994f))
* auto-verify gate — agent runs the project check and fixes its own errors ([9403ff7](https://github.com/Artaeon/nerve/commit/9403ff752566c64dc11c98341526463c4d47874c))
* block agent writes to persistence/exfil targets ([24da644](https://github.com/Artaeon/nerve/commit/24da6446ee395175658b69ac3d6cb445bfd9d645))
* change journal + green-gate commits; split tools god file ([7709124](https://github.com/Artaeon/nerve/commit/7709124122771c4b697d765d154dc53a2da5999e))
* connect the TUI to a remote nerve server with a live job indicator ([0c5f5ac](https://github.com/Artaeon/nerve/commit/0c5f5ac2e52efd6a19665cfd5f38863b2bec33b1))
* context meter shows the real sent-payload size, not raw stored history ([fb36ebd](https://github.com/Artaeon/nerve/commit/fb36ebd1ff34c3d83603228dcd6a3de72d56e0f7))
* context-carrying jobs — hand off the full conversation, nothing lost ([c03dce3](https://github.com/Artaeon/nerve/commit/c03dce3f2a5a7c864ba859511f3671bc8863e0f4))
* **context:** make truncation visible instead of silent ([8e87a2e](https://github.com/Artaeon/nerve/commit/8e87a2e00c3a998ce4286104042a81a1df284332))
* **context:** one context builder for both the TUI and the worker ([586794a](https://github.com/Artaeon/nerve/commit/586794acc19a91a80649765a868884f6c2a91a69))
* design presets — one-command curated design systems ([a873b01](https://github.com/Artaeon/nerve/commit/a873b0146bdf976e87f44d6aed9bf9a3aafd6ebc))
* design principles — auto-applied per-project UI/design guidance ([0bee4d2](https://github.com/Artaeon/nerve/commit/0bee4d22c1e03ee9691d62327c602141e065fa5e))
* design-consistency linter — enforce .nerve/design.md principles ([d498351](https://github.com/Artaeon/nerve/commit/d49835194db09087fe2a66f254cd687638b1b7b8))
* detect any git repository as a workspace ([89d1224](https://github.com/Artaeon/nerve/commit/89d122421dc37952c6bd3b97c24e7f3af09b3321))
* detect truncated file writes (dependency-free brace-balance check) ([5f229ee](https://github.com/Artaeon/nerve/commit/5f229ee9bec8d8eb64bd01e676d48087a5bd0786))
* full project sync — schedule a local repo on the server, nothing lost ([fd4b70d](https://github.com/Artaeon/nerve/commit/fd4b70de21ef636a45fce8ada93470e0dba551e7))
* harden dangerous-command check against denylist bypasses ([448f4b3](https://github.com/Artaeon/nerve/commit/448f4b3d7122196fe2c646252ed1e7e45f205c9e))
* headless agent runner + queue worker — the server now RUNS jobs ([a1efebf](https://github.com/Artaeon/nerve/commit/a1efebf68c9e0c3bf0c730a1456ec83eca2ade10))
* **headless:** context compaction — token-efficient long jobs, never lose the task ([c5d48b7](https://github.com/Artaeon/nerve/commit/c5d48b7cd825795f7b47aaadfbbd1bb9a253ac2f))
* **headless:** inject the project's .nerve memory into every job's context ([c975dce](https://github.com/Artaeon/nerve/commit/c975dcedda56885bd2f4ab945ac95a3f116747fd))
* **headless:** notice when the agent is stuck re-reading one file ([b97e9f9](https://github.com/Artaeon/nerve/commit/b97e9f9083a6aced078e01a7c18c36689f3b8ae1))
* make context usage accurate and transparent ([db6a694](https://github.com/Artaeon/nerve/commit/db6a6944d4984d49fa7f656ffb48fd5fc6eb34c2))
* **memory:** semantic activity journal — records what changed & why, not just that a job ran ([62910ca](https://github.com/Artaeon/nerve/commit/62910caf5efb3f1baac876ddf034f4639fc7ced2))
* multi-agent workflow on the server (planner -&gt; coder -&gt; reviewer) ([5a3448b](https://github.com/Artaeon/nerve/commit/5a3448bf0cb1cb36699f9ec468ad2abda1467d9c))
* out-of-box excellence — provider health checks, auto-fallback, workspace-default agent ([b7094f0](https://github.com/Artaeon/nerve/commit/b7094f04be242420ea2e833d0d4026a76fa8a432))
* per-project persistent memory — the .nerve/ directory ([603ebc6](https://github.com/Artaeon/nerve/commit/603ebc671a43cc3da8b9e4b7113d3c921de3b145))
* persistent task backlog + plan-approval gate for /workflow ([79ecb25](https://github.com/Artaeon/nerve/commit/79ecb25f2fdda76d00a7af1b03eacf9529fd6cab))
* prompt- and role-driven model routing (high for planning, small for trivial) ([236e6e2](https://github.com/Artaeon/nerve/commit/236e6e22ccfc403fba545c75d34c6e8d84d09d08))
* **queue:** a job that changed nothing no longer reports "done" ([343969d](https://github.com/Artaeon/nerve/commit/343969da7de57b3b5a86aaab2a21afc105ce8739))
* **queue:** atomic claim, so two workers can never run the same job ([0e393fd](https://github.com/Artaeon/nerve/commit/0e393fda36cec80d344e2d7909b8d8426037b00b))
* reduce UI/UX friction and make failures clearer ([3d80231](https://github.com/Artaeon/nerve/commit/3d80231f9218d54a98e2054c3af4af03eeab16f7))
* retrieval-based project memory — recall over inject ([35a9b2f](https://github.com/Artaeon/nerve/commit/35a9b2f9a6fcab1bb817abca18956994cd9bb193))
* scrollable help, '?' shortcut, bounded scroll, accurate hints ([1f6d94c](https://github.com/Artaeon/nerve/commit/1f6d94c99d94ceb1f660e23e3a1f7b8261abac29))
* server/client job queue — submit coding jobs to a nerve daemon ([d84207f](https://github.com/Artaeon/nerve/commit/d84207f1d633bac23e6e2e78dc0c3f4e58674bb2))
* stopword-filter recall + verify injection seam end-to-end ([4da4455](https://github.com/Artaeon/nerve/commit/4da445538330ebdc1d1c24c1e718d8e3293a1bce))
* **verify:** let .nerve/verify.toml stand alone as the gate ([3afae7e](https://github.com/Artaeon/nerve/commit/3afae7e200448eab92b186e7a15b6ad090eb95f3))
* **verify:** let a project declare an extra gate step (pure layer) ([c5a6928](https://github.com/Artaeon/nerve/commit/c5a69288bf4b773f8ee97eb243d379869f4b4b4c))
* **verify:** run the project's declared extra gate step ([69632d3](https://github.com/Artaeon/nerve/commit/69632d3df0cef1409d42926a5cfd22147241ded8))
* **worker:** auto-requeue a wedged job instead of failing it ([142b035](https://github.com/Artaeon/nerve/commit/142b0353c66177fc11db1d021875e4c270a86298))
* **worker:** defer jobs on provider quota limit instead of failing them ([2670b6f](https://github.com/Artaeon/nerve/commit/2670b6fa15d60430549ae82b30610b9252c3b6a6))
* **worker:** deterministic sampling by default for unattended runs ([9ce1d8e](https://github.com/Artaeon/nerve/commit/9ce1d8eeb73d921cb445e1f82c5be0079eb49798))
* **worker:** flag cap-stopped jobs as INCOMPLETE in commit + journal ([6595590](https://github.com/Artaeon/nerve/commit/6595590a6c77348d7ab31adaa3f0fddf3c9d21bd))
* **worker:** reclaim orphaned Running jobs on startup ([4064111](https://github.com/Artaeon/nerve/commit/40641116d3ac0677679b61f18c1f3bec92fb78fa))
* **worker:** resume from the attached session context, not just the prompt ([1d87e26](https://github.com/Artaeon/nerve/commit/1d87e2649198b98e47b3b7fcacc7d1151ff0f6ec))
* **worker:** run the project's test suite in the verify gate, not just typecheck ([a7b7452](https://github.com/Artaeon/nerve/commit/a7b74524647c693cf468b2cbc52cc3afb93d1f1a))
* **worker:** verify gate + memory journaling — server jobs now self-correct ([f38d244](https://github.com/Artaeon/nerve/commit/f38d244061d4bdb8c6f7dd446ced7f51ee4ff271))
* **workflow:** sharper reviewer (gets the diff) + read-only prompts + TUI flag ([dc8cc5b](https://github.com/Artaeon/nerve/commit/dc8cc5b10f5d573ed180ee80de2eef98a18f2221))


### Bug Fixes

* agent file edits failed out of the box (--tools) + narrow-terminal panic ([5566a7c](https://github.com/Artaeon/nerve/commit/5566a7c1d36856bb18774abe45d6009c758fa6c4))
* **agent:** a failing build is not a wedged worker ([92e0ad7](https://github.com/Artaeon/nerve/commit/92e0ad7fc80db3bb01d7d3eaf9a2f3e995141560))
* **agent:** escalating explore-nudge + log run_command arguments ([481a9fa](https://github.com/Artaeon/nerve/commit/481a9fa0be5a1d1e149cd9ed3a2786c63e0af70b))
* **agent:** make compaction actually reach its budget ([0022b13](https://github.com/Artaeon/nerve/commit/0022b1358aa8b6459f83f38c7424cf54178f56a7))
* **agent:** make decompose progress durable across a mid-run wedge ([adf1ad2](https://github.com/Artaeon/nerve/commit/adf1ad2c605acb0c903ace1a230f1b03d13ae9a1))
* **agent:** never re-run a decompose step the child already executed ([0ca28e4](https://github.com/Artaeon/nerve/commit/0ca28e494eef203a5ec38ceea8ce26f0f55d8ccc))
* **agent:** stop truncating tool feedback below the read_file cap ([833dc3e](https://github.com/Artaeon/nerve/commit/833dc3e5ffb8666a256ec5ac0237cda64157fe8b))
* **agent:** THE WEDGE — reset the global tool counter per run ([3919ecd](https://github.com/Artaeon/nerve/commit/3919ecd010d288debb0ae0af45860127286f9f1c))
* block path traversal in create_directory and /template ([08c8c45](https://github.com/Artaeon/nerve/commit/08c8c45e0780f0a4df050357ac60e083184c9087))
* bound network and file reads against hangs and OOM ([5959bbb](https://github.com/Artaeon/nerve/commit/5959bbb4f45e55b7a169cd5c5c70611745641dbd))
* **claude_code:** pass the prompt via stdin, not argv — unblocks large contexts ([bc4b487](https://github.com/Artaeon/nerve/commit/bc4b487b7bf9fb48e6985748ec5041fca5f437a1))
* **claude_code:** retry transient failures on the default provider ([f30440c](https://github.com/Artaeon/nerve/commit/f30440cda024f7721e9d9d9f73207a5fc880d4e6))
* close 5 confirmed findings from adversarial branch review ([4871944](https://github.com/Artaeon/nerve/commit/487194476e43952725aba5ca649a8267eb382848))
* close clipboard perms window and stabilize daemon socket path ([c2a4946](https://github.com/Artaeon/nerve/commit/c2a49465a25c4b868937320a93d1f88b3186d105))
* close SSRF bypasses and bound response size in web scraper ([738d4f0](https://github.com/Artaeon/nerve/commit/738d4f018700c6277b11a7e07f311b0be58aac7d))
* close SSRF via HTTP redirects in the web scraper ([ce796b3](https://github.com/Artaeon/nerve/commit/ce796b3b0fc45d36fd4059be00279de134959fa5))
* **context:** pin the original request through interactive compaction ([a9cb470](https://github.com/Artaeon/nerve/commit/a9cb470b709bec920c7686ee32a5a62fa17b5abb))
* **context:** stop throwing away 81% of a project's memory ([f47d952](https://github.com/Artaeon/nerve/commit/f47d952fd4d14d8c7ee8ee0fa53c1f32ebe9c40e))
* correctness bugs in agent turns found by code review ([4c84dde](https://github.com/Artaeon/nerve/commit/4c84ddeec005975d5083cd2c549d4f698f67232a))
* **daemon:** remove the socket on __SHUTDOWN__ so no stale nerve.sock is left ([17576c0](https://github.com/Artaeon/nerve/commit/17576c01db3da5972bf4bd38c2955ddc293e5a2f))
* don't redact 'sk-' inside ordinary words ([fcb1eef](https://github.com/Artaeon/nerve/commit/fcb1eef4d00c4600d82a13430f7e5d36dccd69b0))
* expand [@file](https://github.com/file) once per turn and share the message builder ([ae7d921](https://github.com/Artaeon/nerve/commit/ae7d921b07a41d6d673160b4d13d43f798c67da0))
* harden chat rendering against huge and hostile content ([8c74fa5](https://github.com/Artaeon/nerve/commit/8c74fa5067566f115ad1d30e38911045ae744729))
* harden clipboard rendering, denylist, and daemon socket ([65ce8e6](https://github.com/Artaeon/nerve/commit/65ce8e61cd3ad9348d1b6ee36a3f73e73ec69734))
* harden model-routing classifier (word boundaries + scope, drop length rule) ([a8297ee](https://github.com/Artaeon/nerve/commit/a8297ee541b3c122760fcaf0ca7180751c64ee4a))
* **headless:** be decisive, more iterations, per-iteration logging ([13c403d](https://github.com/Artaeon/nerve/commit/13c403d8275e2a4e41fdd76b3880c2f128726f5e))
* **headless:** nudge a full-tool agent that replies without acting ([b919493](https://github.com/Artaeon/nerve/commit/b919493a18b1b582b3a6c2adbe57bf1354207353))
* **headless:** only flag 'edited' when a mutating tool actually SUCCEEDS ([7f93969](https://github.com/Artaeon/nerve/commit/7f93969f127907f067d2fbd48b8c71ec9218a5ef))
* **headless:** stop the model confabulating a 'tool execution limit' and giving up ([1e4f68b](https://github.com/Artaeon/nerve/commit/1e4f68bd568431dff000625934269d6b17eb60bc))
* keep the event loop responsive and don't deadlock plugins ([c7a1d3b](https://github.com/Artaeon/nerve/commit/c7a1d3b2e1bc84496ffd18865815a6e9919b0133))
* let / insert character in Insert mode instead of opening Nerve Bar ([a45d149](https://github.com/Artaeon/nerve/commit/a45d14924a2d8f7e5364fae55d9dc92675b5bd8d))
* make design linter's off-grid-spacing context-aware ([af8ea36](https://github.com/Artaeon/nerve/commit/af8ea36e512eba1d10736d982ac9293c88d7a10a))
* make on-disk state durable and corruption-resistant ([44b1c48](https://github.com/Artaeon/nerve/commit/44b1c48ef100c67ae694daf8feae77b9cb46b8fc))
* make OpenAI-compatible SSE streaming robust ([8fca277](https://github.com/Artaeon/nerve/commit/8fca27738b64eb26cc34032bbdf889d59d36332f))
* **memory:** stop first_sentence panicking on a multi-byte summary ([8272774](https://github.com/Artaeon/nerve/commit/82727745d686a63ef464f5bd94e28a96dde0838e))
* never lose a turn — persist conversation before streaming + session per turn ([5526f95](https://github.com/Artaeon/nerve/commit/5526f95ab908d8dd9a78c12a2adfe8c9ff963801))
* never touch the real OS clipboard or data dir from tests ([694b158](https://github.com/Artaeon/nerve/commit/694b1581c66357378dc2dd9e2c4d4f1722df21ad))
* **parse:** stop destroying markdown code fences in file content ([f8ff19d](https://github.com/Artaeon/nerve/commit/f8ff19d1c0f0751b72f99ab0a4370bf3d42d8128))
* **parse:** stop re-reading file content as protocol control ([a3318bc](https://github.com/Artaeon/nerve/commit/a3318bc0392a3c0fa72f57826d352cdccb52a590))
* **parse:** stop reading file content as tool arguments ([0cde6c8](https://github.com/Artaeon/nerve/commit/0cde6c8e9a66e479bb4d826ab7264476d7c54834))
* preserve context integrity during compaction ([9357681](https://github.com/Artaeon/nerve/commit/9357681fe852d22a29787c514bb6f9a9aec96f74))
* prevent panics on multibyte and edge-case input ([f5165fd](https://github.com/Artaeon/nerve/commit/f5165fd9f0be1c88a975d7a02e78c0fa35854f4d))
* prevent u16 overflow in popup sizing on wide terminals ([867217a](https://github.com/Artaeon/nerve/commit/867217aba506fc29730ba60b25c43aa7f9cb4490))
* **project:** keep nerve's append-only logs out of git so a reset can't wipe them ([83ab87e](https://github.com/Artaeon/nerve/commit/83ab87edafd1c0f16fb0dbeebbca2053b02fd7de))
* **search:** real BM25 scoring so detailed memories can be recalled ([98c5497](https://github.com/Artaeon/nerve/commit/98c54977cb0c1954d723d7949f1d0bd42883626d))
* **security:** tests no longer kill the live daemon or run sudo for real ([8d7c894](https://github.com/Artaeon/nerve/commit/8d7c89414a6055655c6a315c6dadfbdba55b8f6e))
* stop agent-mode and workflow state from leaking between turns ([0761bc4](https://github.com/Artaeon/nerve/commit/0761bc480897b5bcd3fa45a4f175cc80a7283432))
* stop agent-mode leaks and stream task leaks across turns ([8572a6c](https://github.com/Artaeon/nerve/commit/8572a6cfe3bbe01b383ebb9f36022117d2c06d4b))
* stop auto-context from leaking secret files to the LLM ([c9bbf20](https://github.com/Artaeon/nerve/commit/c9bbf20ff432c67100e3fd3cc0f1997c49f6d2e7))
* **sync:** protect the server's .git from --delete so job branches survive ([4e6e92f](https://github.com/Artaeon/nerve/commit/4e6e92f88dfa9be8b9546800ac97bfc8a67de668))
* **tools:** allow Next.js dynamic routes in write paths ([a11517a](https://github.com/Artaeon/nerve/commit/a11517a0c1492806ac83f51359662a6e3f5438ab))
* **tools:** refuse to write a file whose path is a line of source code ([1fa1c33](https://github.com/Artaeon/nerve/commit/1fa1c331b9b570f61fbc678756c60a6f7966a8d2))
* **verify:** decide watch-mode on flags, not on the substring "watch" ([f5b73a7](https://github.com/Artaeon/nerve/commit/f5b73a798cd228da3ec3c6f0792778f1d49a63b6))
* **verify:** give the verify gate a timeout ([fb87069](https://github.com/Artaeon/nerve/commit/fb87069053c89140a420415ce78df47757dfe10a))
* **verify:** pick the verify command from scripts, not the whole package.json ([26e3a73](https://github.com/Artaeon/nerve/commit/26e3a73ff72ebc73dd7e8ff201aa3c9ffb6f47cc))
* **worker:** a --decompose job that committed its work isn't "no-changes" ([6b6cdaf](https://github.com/Artaeon/nerve/commit/6b6cdafcd949a10e400f0026f7b7408e58912a3f))
* **worker:** a job that ran out of steps mid-task is not `done` ([9962ab8](https://github.com/Artaeon/nerve/commit/9962ab83b077f788ee7e2bcf856cac63e151d43c))
* **worker:** commit only the files the job changed, never git add -A ([587c816](https://github.com/Artaeon/nerve/commit/587c8164bf6911cf6206197ca5261133b74111dc))
* **worker:** defer a quota'd job to the provider's stated reset time ([e611ca7](https://github.com/Artaeon/nerve/commit/e611ca792019434e5dbcd2c8aec75ff0cc2c689c))
* **worker:** fork every job from a clean base branch, not the previous job's ([7fea45a](https://github.com/Artaeon/nerve/commit/7fea45ac0e6d0b8739959119eeba8995719a12ee))
* **worker:** never run a job unless HEAD is really on its branch ([ca51ddc](https://github.com/Artaeon/nerve/commit/ca51ddc0fe10ca8e0945394cda34d54039b78b8e))
* **worker:** never stage generated build output into a job's commit ([56fc399](https://github.com/Artaeon/nerve/commit/56fc3997a3acbd8752dfe1b768cf804b34ed6ec9))
* **worker:** proactively recycle the process every N jobs to pre-empt the wedge ([c0084fc](https://github.com/Artaeon/nerve/commit/c0084fcb26bfad4f18609cdc6b49bb287ce3e1aa))
* **worker:** restore the verify-gate timeout, deleted by an unrelated commit ([5f64593](https://github.com/Artaeon/nerve/commit/5f645933913f0bd80c6a87d6fc3c45b75b99312a))
* **worker:** self-heal a wedged worker instead of failing every job silently ([9b60ad7](https://github.com/Artaeon/nerve/commit/9b60ad75847a5ae025a6533b1b6e3ca7ae15d78d))
* **worker:** start every job from a pristine tree, not just a base checkout ([d19afd3](https://github.com/Artaeon/nerve/commit/d19afd3720a68fff2ee84e5d4256106f8b8f1f95))
* **worker:** stop deleting the project's own memory before every job ([86fd517](https://github.com/Artaeon/nerve/commit/86fd517ca7eeaf86b8a9554c2cd4ed17ced4f6ec))
* **worker:** stop reporting `done` for code the verify gate rejected ([22385b3](https://github.com/Artaeon/nerve/commit/22385b305b94f16b64fbe7098d7168f05ae1b2af))
* **worker:** trust synced repos in git (safe.directory) so branch isolation works ([060502e](https://github.com/Artaeon/nerve/commit/060502e051de9e9a6957370b3581fd54a8a0faf2))
* **workflow:** don't run a fix round when the reviewer ran out of iterations ([9ad1bb8](https://github.com/Artaeon/nerve/commit/9ad1bb8da7bd922ae0b73f1656451577c77799c1))


### Performance Improvements

* **agent:** collapse identical re-reads instead of re-sending the file ([6799e03](https://github.com/Artaeon/nerve/commit/6799e032ad610f0dca2d30db4319eea44cb6e6b7))
* **headless:** nudge the agent to implement after long read-only exploration ([82ab475](https://github.com/Artaeon/nerve/commit/82ab475ef1d573c9c58e266dbf7ba77cba482730))

## [Unreleased]

### Added

#### Out-of-the-box experience
- Provider health checks at startup (PATH scan for `claude`/`gh`, TCP probe for Ollama, key presence for OpenAI/OpenRouter) with automatic fallback to the best available provider when the default can't run
- Friendly multi-provider setup guidance when no provider is available, instead of a raw error on the first prompt
- Workspace-default agent activation: inside a detected project, coding requests activate the agent automatically; clearly conversational messages stay chat-only (`intent::should_activate_agent`)
- Any git repository is now detected as a workspace even without a language manifest; language inferred from the dominant source-file extension
- Claude CLI failures surface the CLI's own message with login guidance instead of a raw JSON blob

#### Per-project persistent memory (`.nerve/`)
- `/init` -- analyze the repo once and save an engineering brief injected into every prompt
- `/remember <fact>`, `/memory` -- persist and view project facts/conventions
- `/decision <text>`, `/decisions` -- append-only decision log (last 5 always in context)
- `/task <title>`, `/tasks`, `/task done|start|fail <id>` -- a task backlog that survives sessions
- `/improve <idea>`, `/improvements` -- improvement backlog
- `/changes` -- audit trail (`.nerve/journal.jsonl`) of every agent file write
- New agent tools `remember` and `update_tasks` (12 tools total) so the model maintains memory/tasks itself
- `.nerve/` is write-protected from the agent's file tools; all writes go through a sanitized API (prompt-injection persistence defense)

#### Multi-agent workflow
- Plan-approval gate: `/workflow` now pauses after planning -- nothing executes until you `/approve` (or `/reject`); `workflow_auto_approve` config restores the old behavior
- Planner runs with read-only repo access so plans reference real files and symbols
- Green-gate commits: `/agent commit` runs the project's tests first and refuses to commit on red; `/agent commit force` overrides

#### Developer Workflow
- `/lint` command -- auto-detect and run project linter (clippy, eslint, ruff, golangci-lint, rubocop, credo)
- `/format` (`/fmt`) command -- auto-detect and run code formatter (cargo fmt, prettier, ruff, gofmt, rubocop, mix format)
- `/search <pattern>` command -- search codebase with ripgrep, results added as AI context
- `/commit` now uses `--author` from configured git_user_name/email
- AI-generated commit messages via `/commit` (no message argument)

#### AI & Prompt Engineering
- `temperature` config option (0.0-2.0) for controlling response creativity
- `top_p` config option (0.0-1.0) for nucleus sampling
- `context_limit` config option to override provider default context window size
- Mode-specific system prompts: Efficient (concise), Thorough (detailed), Agent (workflow), Learning (Socratic)
- Ollama default context raised from 8K to 32K tokens

#### UI/UX
- Color-coded token usage percentage in status bar (green/yellow/red)
- Vim `G`/`g` keys to jump to bottom/top of conversation
- `PageUp`/`PageDown` for fast scrolling (30 lines)
- Context-aware status bar hints (different hints per mode)
- Working directory display in status bar when code mode is active
- Auto-agent mode: automatically enables tools when message needs them

### Fixed

#### Plan-approval gate & workflow hardening (adversarial review)
- **Approval-gate bypass**: a parked workflow is now advanced only by an explicit `/approve`; the event-loop no longer executes the plan when an unrelated message is sent while awaiting approval
- Ordinary messages are blocked with guidance while a workflow awaits approval
- `remember` and `update_tasks` are now treated as write tools, so read-only roles (the pre-approval Planner, the Reviewer) cannot mutate `.nerve/` project memory
- `Esc` cancels a workflow parked at the approval gate; `/clear` tears the pipeline down like `/new`
- `/agent commit` green gate uses a full-suite timeout (600s) instead of the 30s per-tool timeout, so real test suites no longer falsely "time out"

#### Security Hardening
- **SSRF**: Proper URL parsing replaces string-matching blocklist; blocks IPv6 loopback, link-local, ULA (fc00::/7), multicast (ff00::/8), IPv4-mapped IPv6, IPv4-compatible IPv6, CGN range
- **SSRF**: Final URL re-validated after redirect chain to prevent redirect-based SSRF
- **Command injection**: All agent tool shell commands use `shell_escape()` (verify_file_syntax, search_code, find_files, git_diff)
- **Path traversal**: `normalize_path()` resolves `..` segments without filesystem access; blocks `/tmp/x/../../etc/passwd`
- **Path traversal**: History conversation IDs sanitized to alphanumeric+hyphens
- **Symlink attacks**: `validate_write_path()` canonicalizes paths before protection check
- **Device file DoS**: Block char/block devices, FIFOs, sockets, symlinks from file reads
- **Command filter**: Block poweroff, halt, systemctl reboot, shred, wipe, chpasswd; block pipe to zsh, python, perl, ruby, ksh, dash
- **ANSI injection**: Strip ANSI escape sequences (CSI, OSC, SS2/SS3) from plugin output
- **Clipboard**: Propagate permission-setting errors instead of silently ignoring
- **Knowledge search**: Fix ln(0) edge case when scoring empty chunks
- **Memory**: `truncate_output()` avoids collecting all lines before truncating
- **SSRF (redirects)**: Redirects are followed manually with per-hop resolve-and-pin — every hop is re-validated against private IPs, closing DNS-rebind/redirect SSRF that a string-only final-URL check missed
- **Secret-file leak**: Auto-context no longer reads `.env`/`id_rsa`/`.ssh`/credential files and injects them to the LLM (now matches the `read_file` tool guard)
- **Path traversal**: `create_directory` and `/template` are now validated (previously bypassable with `..`/absolute paths)
- **Persistence/exfil targets**: Agent writes are blocked to `~/.ssh/authorized_keys`, `~/.ssh/config`, shell rc files, `.git/hooks/*`, and credential files (`.aws/credentials`, `.netrc`, `.pgpass`) — the classic prompt-injection targets
- **Dangerous-command denylist**: Structural `rm -r -f` detection resists flag-order / long-flag / path-prefixed-binary bypasses (`rm --recursive --force /`, `/bin/rm -rf /`)
- **Daemon**: Control socket moved off world-shared `/tmp/nerve.sock` to a per-user `~/.nerve` dir (0700) with a 0600 socket, HOME-anchored for a deterministic client/daemon path
- **Clipboard**: History file written 0600 atomically with no world-readable window; tests never touch the real OS clipboard or data dir

#### Reliability & Context
- **@file expansion**: Expanded once, on the latest user turn only — was re-reading each referenced file (up to 1 MB) and re-injecting it on *every* request (quadratic token growth); compaction now runs on the true payload
- **Shared message builder**: The initial send, post-tool follow-up, and regenerate all build context the same way, so the active mode/persona, knowledge-base results, auto-context, and `@file` content are never silently dropped after a tool round or on regenerate
- **Compaction**: Preserves chronological order and no longer hoists (or silently drops) mid-stream file/command context; notifies the user when it summarizes older turns
- **Auto-agent**: Reliably reverts after a tool-running turn (no leak into the next plain message); injected project context no longer accumulates across activations
- **Token accounting**: Unified estimator across the status bar, `/tokens`, `/context`, the spending check, and compaction (was inconsistent — the on-screen % was ~25% off)
- Panic fixes on multibyte / edge-case input; bounded network and file reads; concurrent plugin pipe draining (fixes >64 KB output deadlock); u16 overflow fix in popup sizing on wide terminals

#### UI/UX
- Welcome/onboarding screen now appears on the common in-workspace first run
- Typo'd slash commands are caught (`Unknown command …`) instead of being sent to the model
- Friendly, provider-specific setup help on first run when no key/CLI/local server is configured
- Errors/warnings linger longer than transient confirmations; `Esc: stop` hint during streaming; scrollable help overlay; bounded scroll

### Changed
- Removed dead `/commit` handler from shell.rs (git.rs handles it)
- Moved `build_git_author_flag` to git.rs where it's used
- Split god files into cohesive modules: `main.rs` 5,100 → 2,280 lines (extracted `splash`, `provider_setup`, `completion`, `conversation`, `input`); extracted `ui::markdown`, `ui::{theme,selectors,status_bar,input_box}`, and `agent::tools::parse`

### Quality
- Tests: 1,345 -> 1,760+
- 0 clippy warnings (`-D warnings`)
- 0 formatting issues
- Extensive additional security, context-management, and reliability hardening; no god files remaining

## [0.1.0] - 2025-04-01

### Added

#### AI Providers
- Claude Code (no API key needed — uses subscription)
- OpenAI (API key)
- OpenRouter (API key, 100+ models)
- Ollama (local, free, offline)
- GitHub Copilot (gh CLI)
- Custom OpenAI-compatible endpoints

#### Chat
- Streaming responses with syntax-highlighted code blocks
- Markdown rendering (bold, italic, headers, lists, blockquotes, links)
- Line numbers in code blocks
- Animated thinking spinner and streaming progress bar
- Message number badges (1-9) for quick copy
- Multi-line input with Shift+Enter
- Dynamic input area that grows with content
- Input history with Up/Down arrows
- Vim keybindings (i/Esc, j/k scroll)
- Mouse scroll support

#### Agent Mode
- 9 tools: read_file, write_file, edit_file, run_command, list_files, search_code, create_directory, find_files, read_lines
- Plan-execute-observe loop (max 10 iterations)
- Robust tool call parsing (handles XML tags, JSON, markdown fences, missing closing tags)
- Auto-verify file syntax after edits (Rust, Python, JS, JSON, YAML, TOML)
- Git safety net (auto-stash on start, /agent undo to rollback)
- Project map injection (file tree + key symbols)
- Provider-aware context compaction
- Works with any provider (not just Claude Code)

#### Smart Prompts
- 166 expert prompts across 28 categories
- Categories: Engineering, Coding, Writing, Design, Git, Rust, Python, TypeScript, Go, UI/UX, Testing, Security, API, Database, Cloud, DevOps, Business, Marketing, and more
- Each prompt is 5-15 lines of detailed system instructions
- Nerve Bar (Ctrl+K) with fuzzy search, category tabs, and template preview
- Custom prompts via TOML files

#### Developer Workflow
- File context: /file, /files, @file inline references
- Shell integration: /run, /test, /build, /diff, /pipe, /git
- Workspace detection (Rust, Node, Python, Go, Java, Ruby, Elixir, Zig, C#, C++)
- Project scaffolding: 8 templates + AI-generated /scaffold
- Tab completion for file paths and slash commands

#### Productivity
- Persistent sessions (auto-save, resume with --continue)
- Conversation branching (/branch save, restore, diff)
- Conversation history browser (Ctrl+O) with search, sort, delete confirmation
- Clipboard manager (Ctrl+B) with fuzzy search
- In-chat search (Ctrl+F)
- Stop (Esc), regenerate (Ctrl+R), edit (Ctrl+E) responses
- Export conversations as markdown
- Knowledge base (RAG) with document ingestion
- Automations (5 built-in multi-step pipelines)
- Command aliases (/alias)

#### Settings & Customization
- Interactive settings overlay (Ctrl+,) with 4 tabs
- 10 color themes (Catppuccin, Tokyo Night, Gruvbox, Nord, Solarized, Dracula, One Dark, Rose Pine, High Contrast, Monochrome)
- TOML configuration at ~/.config/nerve/config.toml
- Plugin system (executable scripts in ~/.config/nerve/plugins/)

#### Token Management
- Provider-aware context limits (Claude 200K, OpenAI 60K, OpenRouter 30K, Ollama 32K)
- Auto-compaction with smart summarization
- Tool result compaction in agent mode
- Usage tracking (/usage) and spending limits (/limit)
- max_tokens on OpenAI/OpenRouter requests

#### Security
- Shell injection blocking (30+ dangerous patterns)
- Protected system paths (agent can't write to /etc, /usr, /bin, etc.)
- Sensitive file blocking (.env, SSH keys, .aws/credentials)
- Tool execution rate limiting (100/session)
- Config file permissions (0600 on Unix)
- API key masking in display

#### CLI
- Non-interactive mode: nerve -n "prompt"
- Pipe mode: cat file | nerve --stdin -n "review"
- Daemon mode: nerve --daemon / --query / --stop-daemon
- Resume sessions: nerve --continue
- Provider/model override: nerve --provider ollama --model llama3

#### Quality
- 820 tests across 42 source files (now 1,345 tests across 55 files)
- 0 clippy warnings
- 0 unsafe code
- 0 panics in production code
- Graceful error handling with helpful messages
- CI/CD with GitHub Actions (build, test, clippy, auto-release)

[Unreleased]: https://github.com/Artaeon/nerve/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Artaeon/nerve/releases/tag/v0.1.0
