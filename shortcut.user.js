// ==UserScript==
// @name        nixpkgs-review-gha repository review
// @match       https://github.com/*
// @run-at      document-idle
// ==/UserScript==

// Explicitly configure in the browser console:
// localStorage.setItem("repo-review-controller", "OWNER/CONTROLLER-REPOSITORY")
// No owner or repository-name assumption is made.
const controller = localStorage.getItem("repo-review-controller");
const coordinate = /^[A-Za-z0-9_-]+\/[A-Za-z0-9_.-]+$/;

function setup() {
  if (!controller || !coordinate.test(controller)) return;
  const pr = /^\/([^/]+\/[^/]+)\/pull\/([1-9][0-9]*)(?:\/.*)?$/.exec(location.pathname);
  if (!pr || !coordinate.test(pr[1])) return;
  const actions = document.querySelector("div[data-component=PH_Actions], .gh-header-show .gh-header-actions");
  if (!actions || actions.querySelector(".repo-review-shortcut")) return;
  const button = document.createElement("button");
  button.className = "Button Button--secondary Button--small repo-review-shortcut";
  button.textContent = "Review with Nix";
  button.onclick = () => {
    const nixpkgs = pr[1].toLowerCase() === "nixos/nixpkgs";
    const request = {
      schema_version: 1,
      repository: pr[1],
      pr: Number(pr[2]),
      mode: "head",
      backend: nixpkgs ? "nixpkgs" : "flake",
      systems: ["x86_64-linux"],
      packages: nixpkgs ? [] : ["default"],
      checks: [],
      publication: "none",
      post_result: false,
    };
    const edited = prompt(
      "Review/edit explicit outputs or supply an external-flake recipe. This does not dispatch automatically.",
      JSON.stringify(request),
    );
    if (!edited) return;
    try {
      const parsed = JSON.parse(edited);
      window.open(
        `https://github.com/${controller}/actions/workflows/review-repository.yml#request=${encodeURIComponent(JSON.stringify(parsed))}`,
        "_blank",
        "noopener",
      );
    } catch {
      alert("Invalid JSON; nothing dispatched.");
    }
  };
  actions.prepend(button);
}

async function prefill() {
  if (!controller || location.pathname !== `/${controller}/actions/workflows/review-repository.yml`) return;
  const value = new URLSearchParams(location.hash.slice(1)).get("request");
  if (!value) return;
  // Bounded convenience only: GitHub owns this DOM and may change it.
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    const input = document.querySelector('[name="inputs[request]"]');
    if (input) {
      input.value = value;
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.focus();
      return;
    }
    await new Promise(resolve => setTimeout(resolve, 250));
  }
  prompt("Open Run workflow and paste this request into its request field:", value);
}

new MutationObserver(setup).observe(document, { subtree: true, childList: true });
setup();
prefill();
