"""Deterministic GitHub PR feedback helper for the resolve-pr-feedback skill.

Wraps the GitHub CLI (`gh`, REST and GraphQL) to fetch unresolved pull request
review feedback and to reply to / resolve threads or post a summary comment.
Judgment about *how* to fix flagged code stays with the calling agent; this
script only handles the mechanical GitHub API calls so that part of the
workflow is deterministic and repeatable.

Usage:
    python pr_feedback.py fetch   --repo OWNER/NAME --pr NUMBER
    python pr_feedback.py reply   --repo OWNER/NAME --pr NUMBER --comment-id ID (--body TEXT | --body-file PATH)
    python pr_feedback.py resolve --thread-id THREAD_ID
    python pr_feedback.py comment --repo OWNER/NAME --pr NUMBER (--body TEXT | --body-file PATH)

Requires the GitHub CLI (`gh`) installed and authenticated (`gh auth status`).
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys

THREADS_QUERY = """
query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      reviewThreads(first: 100, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id
          isResolved
          isOutdated
          path
          line
          comments(first: 50) {
            nodes { id databaseId body author { login } createdAt url }
          }
        }
      }
    }
  }
}
"""

RESOLVE_MUTATION = """
mutation($id: ID!) {
  resolveReviewThread(input: { threadId: $id }) {
    thread { id isResolved }
  }
}
"""


def run(args: list[str]) -> str:
    # check=False because the returncode is handled explicitly below; letting
    # subprocess raise would lose the stderr this writes out first.
    proc = subprocess.run(args, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(proc.returncode)
    return proc.stdout


def gh_json(args: list[str]):
    return json.loads(run(["gh", *args]))


def graphql(query: str, **fields: str):
    args = ["api", "graphql", "-f", f"query={query}"]
    for key, value in fields.items():
        args += ["-F", f"{key}={value}"]
    return gh_json(args)


def resolve_body(args: argparse.Namespace) -> str:
    if args.body_file:
        with open(args.body_file, encoding="utf-8") as handle:
            return handle.read()
    return args.body


def fetch(repo: str, pr: int) -> None:
    owner, name = repo.split("/", 1)

    pr_view = gh_json(
        ["pr", "view", str(pr), "--repo", repo, "--json", "headRefName,headRefOid,url,title,state"]
    )
    head_sha = pr_view["headRefOid"]
    head_commit_date = gh_json(["api", f"repos/{repo}/commits/{head_sha}"])["commit"]["committer"]["date"]
    me = run(["gh", "api", "user", "-q", ".login"]).strip()

    threads: list[dict] = []
    cursor = None
    while True:
        variables = {"owner": owner, "repo": name, "number": pr}
        if cursor:
            variables["cursor"] = cursor
        data = graphql(THREADS_QUERY, **variables)
        review_threads = data["data"]["repository"]["pullRequest"]["reviewThreads"]
        for node in review_threads["nodes"]:
            if node["isResolved"]:
                continue
            comments = node["comments"]["nodes"]
            threads.append(
                {
                    "thread_id": node["id"],
                    "path": node["path"],
                    "line": node["line"],
                    "outdated": node["isOutdated"],
                    "first_comment_id": comments[0]["databaseId"] if comments else None,
                    "comments": [
                        {
                            "author": comment["author"]["login"] if comment["author"] else None,
                            "body": comment["body"],
                            "created_at": comment["createdAt"],
                            "url": comment["url"],
                        }
                        for comment in comments
                    ],
                }
            )
        page_info = review_threads["pageInfo"]
        if not page_info["hasNextPage"]:
            break
        cursor = page_info["endCursor"]

    issue_comments = gh_json(["api", f"repos/{repo}/issues/{pr}/comments", "--paginate"])
    recent_issue_comments = [
        {
            "author": comment["user"]["login"],
            "body": comment["body"],
            "created_at": comment["created_at"],
            "url": comment["html_url"],
        }
        for comment in issue_comments
        if comment["created_at"] > head_commit_date and comment["user"]["login"] != me
    ]

    reviews = gh_json(["api", f"repos/{repo}/pulls/{pr}/reviews", "--paginate"])
    recent_reviews = [
        {
            "author": review["user"]["login"],
            "state": review["state"],
            "body": review["body"],
            "submitted_at": review["submitted_at"],
        }
        for review in reviews
        if review["body"] and review["submitted_at"] > head_commit_date and review["user"]["login"] != me
    ]

    print(
        json.dumps(
            {
                "repo": repo,
                "pr": pr,
                "title": pr_view["title"],
                "head_branch": pr_view["headRefName"],
                "head_sha": head_sha,
                "head_commit_date": head_commit_date,
                "unresolved_threads": threads,
                "recent_issue_comments": recent_issue_comments,
                "recent_reviews": recent_reviews,
            },
            indent=2,
        )
    )


def reply(repo: str, pr: int, comment_id: str, body: str) -> None:
    run(["gh", "api", f"repos/{repo}/pulls/{pr}/comments/{comment_id}/replies", "-f", f"body={body}"])
    print(f"replied to comment {comment_id} on {repo}#{pr}")


def resolve(thread_id: str) -> None:
    graphql(RESOLVE_MUTATION, id=thread_id)
    print(f"resolved thread {thread_id}")


def comment(repo: str, pr: int, body: str) -> None:
    run(["gh", "pr", "comment", str(pr), "--repo", repo, "--body", body])
    print(f"commented on {repo}#{pr}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    subparsers = parser.add_subparsers(dest="cmd", required=True)

    fetch_parser = subparsers.add_parser("fetch", help="Fetch unresolved PR feedback as JSON")
    fetch_parser.add_argument("--repo", required=True, help="owner/name")
    fetch_parser.add_argument("--pr", required=True, type=int)

    reply_parser = subparsers.add_parser("reply", help="Reply to a review comment thread")
    reply_parser.add_argument("--repo", required=True, help="owner/name")
    reply_parser.add_argument("--pr", required=True, type=int)
    reply_parser.add_argument("--comment-id", required=True, help="first_comment_id from fetch")
    body_group = reply_parser.add_mutually_exclusive_group(required=True)
    body_group.add_argument("--body")
    body_group.add_argument("--body-file")

    resolve_parser = subparsers.add_parser("resolve", help="Mark a review thread resolved")
    resolve_parser.add_argument("--thread-id", required=True, help="thread_id from fetch")

    comment_parser = subparsers.add_parser("comment", help="Post a general PR comment")
    comment_parser.add_argument("--repo", required=True, help="owner/name")
    comment_parser.add_argument("--pr", required=True, type=int)
    comment_body_group = comment_parser.add_mutually_exclusive_group(required=True)
    comment_body_group.add_argument("--body")
    comment_body_group.add_argument("--body-file")

    args = parser.parse_args()

    if args.cmd == "fetch":
        fetch(args.repo, args.pr)
    elif args.cmd == "reply":
        reply(args.repo, args.pr, args.comment_id, resolve_body(args))
    elif args.cmd == "resolve":
        resolve(args.thread_id)
    elif args.cmd == "comment":
        comment(args.repo, args.pr, resolve_body(args))


if __name__ == "__main__":
    main()
