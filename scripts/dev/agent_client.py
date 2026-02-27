#!/usr/bin/env python3
"""
Minimal claude-agent-sdk exploration client.

Uses the `query` API for one-shot prompts: sends a prompt to Claude,
streams back the message events, and prints each one with its type.

Usage:
    CLAUDECODE= python3 scripts/agent_client.py
    CLAUDECODE= python3 scripts/agent_client.py "your prompt here"
    CLAUDECODE= python3 scripts/agent_client.py --tools "your prompt"

The CLAUDECODE= prefix is required: the SDK spawns a nested claude CLI
process, which Claude Code blocks by default via the CLAUDECODE env var.

Known limitation (SDK v0.1.39):
    The Anthropic streaming API sends a `rate_limit_event` SSE between
    tool calls and tool results. The SDK raises MessageParseError on
    unknown event types, which terminates the stream generator. For
    no-tool prompts the event arrives after the response text, so
    content is received cleanly. For tool-using prompts (--tools mode)
    the stream is cut short after the first tool call.
"""

import sys

import anyio

from claude_agent_sdk import query
from claude_agent_sdk._errors import MessageParseError
from claude_agent_sdk.types import (
    AssistantMessage,
    ClaudeAgentOptions,
    ResultMessage,
    SystemMessage,
    TextBlock,
)


async def run(prompt: str, use_tools: bool) -> None:
    options = ClaudeAgentOptions(
        # No tools by default: rate_limit_event arrives after the response,
        # so we receive the full text. With tools enabled the event lands
        # mid-stream (between tool call and result), truncating the output.
        allowed_tools=["Read", "Glob", "Grep"] if use_tools else [],
        permission_mode="acceptEdits",
        max_turns=5,
    )

    print(f"Prompt:    {prompt!r}")
    print(f"Tools:     {'Read, Glob, Grep' if use_tools else 'none'}")
    print("-" * 60)

    try:
        async for event in query(prompt=prompt, options=options):
            if isinstance(event, SystemMessage):
                print(f"[system/{event.subtype}]")

            elif isinstance(event, AssistantMessage):
                for block in event.content:
                    if isinstance(block, TextBlock):
                        print(f"[assistant] {block.text}")
                    else:
                        kind = getattr(block, "type", type(block).__name__)
                        name = getattr(block, "name", "")
                        print(f"[tool_use/{kind}] {name}")

            elif isinstance(event, ResultMessage):
                cost = event.total_cost_usd
                cost_str = f"${cost:.4f}" if cost is not None else "n/a"
                status = "error" if event.is_error else "ok"
                print(f"[result] turns={event.num_turns}, cost={cost_str}, status={status}")

            else:
                print(f"[{type(event).__name__}]")

    except MessageParseError as e:
        # SDK v0.1.39: rate_limit_event and other unknown SSE types raise
        # MessageParseError, terminating the stream. Non-fatal — log and exit.
        print(f"[parse_error] {e}")

    print("-" * 60)
    print("Done.")


def main() -> None:
    args = sys.argv[1:]
    use_tools = "--tools" in args
    args = [a for a in args if a != "--tools"]
    prompt = (
        " ".join(args)
        if args
        else "Say hello and explain what the claude-agent-sdk is in two sentences."
    )
    anyio.run(run, prompt, use_tools)


if __name__ == "__main__":
    main()
