// ==UserScript==
// @name        nixpkgs-review-gha
// @match       https://github.com/*
// @run-at      document-idle
// ==/UserScript==

const user =
  document.querySelector("meta[name='user-login']")?.getAttribute("content") ??
  document.querySelector("button[data-login]")?.getAttribute("data-login") ??
  document.querySelector("header.GlobalNav button[data-login]")?.getAttribute("data-login") ??
  null;

const repo = user ? `${user}/nixpkgs-review-gha` : null;

const reviewDefaults = ({
  title,
  commits,
  labels,
  author,
  authoredByMe,
  targetBranch,
  hasLinuxRebuilds,
  hasDarwinRebuilds,
}) => {
  const darwinSandbox = "relaxed";

  const hasRebuilds = hasLinuxRebuilds || hasDarwinRebuilds;
  const targetsStableRelease = targetBranch.match(/-\d\d\.\d\d$/);

  return {
    // "branch": "main",
    "x86_64-linux": !hasRebuilds || hasLinuxRebuilds,
    "aarch64-linux": !hasRebuilds || hasLinuxRebuilds,
    "x86_64-darwin":
      targetsStableRelease && (!hasRebuilds || hasDarwinRebuilds) ? `yes_sandbox_${darwinSandbox}` : "no",
    "aarch64-darwin": !hasRebuilds || hasDarwinRebuilds ? `yes_sandbox_${darwinSandbox}` : "no",
    // "riscv64-linux": false,
    // "extra-args": "",
    // "push-to-cache": true,
    // "upterm": false,
    // "post-result": true,
    // "on-success": "nothing",
  };
};

const prTrackers = [
  { name: "nixpk.gs", toUrl: pr => `https://nixpk.gs/pr-tracker.html?pr=${pr}` },
  { name: "ocfox.me", toUrl: pr => `https://nixpkgs-tracker.ocfox.me/?pr=${pr}` },
];

const sleep = duration => new Promise(resolve => setTimeout(resolve, duration));
const query = async (doc, sel) => {
  await sleep(0);
  while (true) {
    const elem = typeof sel === "function" ? sel(doc) : doc.querySelector(sel);
    if (elem) return elem;
    await sleep(100);
  }
};

const getPrDetails = pr => {
  const title = document.querySelector(
    "div[data-component=TitleArea] .text-normal.markdown-title, bdi.js-issue-title.markdown-title",
  ).innerText;
  const targetBranch = document
    .querySelector("[data-component=PH_Title] a[data-component=BranchName]")
    .innerText.replace(/^NixOS:/, "");
  const commits = [...document.querySelectorAll(".TimelineItem-body a.markdown-title[href*='/commits/']")].flatMap(
    ({ title, href }) => {
      const match = /\/NixOS\/nixpkgs\/pull\/(\d+)\/commits\/([0-9a-f]+)$/i.exec(href);
      return match === null || match[1] !== pr
        ? []
        : [
            {
              commit_id: match[2],
              subject: title.split("\n")[0],
              description: title,
            },
          ];
    },
  );
  const labels = [...document.querySelectorAll("div.js-issue-labels > a")].map(x => x.innerText);
  const author = document.querySelector(".js-discussion > :first-child a.author").href.split("/").at(-1);
  const authoredByMe = author === user;
  const hasLinuxRebuilds = !labels.some(l => /rebuild-linux: 0$/.test(l));
  const hasDarwinRebuilds = !labels.some(l => /rebuild-darwin: 0$/.test(l));
  const state = document
    .querySelector("div[data-component=TitleArea] div[data-component=PH_LeadingVisual] span, span.State")
    .innerText.trim()
    .toUpperCase();

  return {
    title,
    commits,
    labels,
    author,
    authoredByMe,
    targetBranch,
    hasLinuxRebuilds,
    hasDarwinRebuilds,
    state,
  };
};

const setupActionsPage = async () => {
  const match = /^https:\/\/github.com\/([^/]+\/[^/]+)\/actions\/workflows\/review.yml#dispatch:(.*)$/.exec(
    location.href,
  );
  if (match === null || match[1] !== repo) return;

  const inputs = new URLSearchParams(match[2]);

  const isMenuOpen = (doc = document) => {
    const openDetails =
      doc.querySelector("details[open] .workflow-dispatch") ||
      Array.from(doc.querySelectorAll("details[open] > summary.btn")).find(s => s.textContent.includes("Run workflow"))
        ?.parentElement;
    if (openDetails) return openDetails;

    return (
      doc.querySelector('[class*="WorkflowDispatchMenu"]') ||
      doc.querySelector('button[data-component="Dialog.FooterButton"][type="submit"]') ||
      null
    );
  };

  if (!isMenuOpen()) {
    const trigger = await query(document, doc => {
      const oldSummary = doc.querySelector("details:not([open]) > summary.btn");
      if (oldSummary && oldSummary.textContent.includes("Run workflow")) return oldSummary;
      return Array.from(doc.querySelectorAll("button")).find(
        b =>
          b.textContent.trim() === "Run workflow" &&
          b.type !== "submit" &&
          !b.closest('[class*="WorkflowDispatchMenu"], [data-component="Dialog"], dialog'),
      );
    });
    trigger.click();
  }

  await query(document, isMenuOpen);

  const setBranchOld = async branch => {
    (await query(document, "details .workflow-dispatch")).classList.add("old-branch");
    document.querySelector("details .workflow-dispatch .branch-selection > details > summary").click();
    (await query(document, `ref-selector[type=branch] button[value="${branch}"]`)).click();
    while ((await query(document, "details .workflow-dispatch")).classList.contains("old-branch")) await sleep(100);
  };

  const setBranchNew = async branch => {
    const branchBtn = document.querySelector(
      'button[data-icv-name="Switch branches/tags"], button[data-testid="anchor-button"][aria-label^="Use workflow from"]',
    );
    if (!branchBtn) return;
    const currentBranch = branchBtn.getAttribute("aria-label") || branchBtn.textContent.trim();
    if (currentBranch.endsWith(`: ${branch}`) || branchBtn.textContent.trim() === branch) return;

    branchBtn.click();
    const targetOption = await query(document, () => {
      return Array.from(
        document.querySelectorAll(
          '[class*="RefSelectorAnchoredOverlay"] button, [data-component="RefSelector"] [role="option"], [data-component="ActionList.Item"]',
        ),
      ).find(el => el.textContent.trim() === branch || el.getAttribute("data-value") === branch);
    });
    targetOption.click();
    await sleep(200);
  };

  const setBranch = async branch => {
    if (document.querySelector("details .workflow-dispatch")) {
      await setBranchOld(branch);
    } else {
      await setBranchNew(branch);
    }
  };

  const setNativeValue = (input, val) => {
    const proto = Object.getPrototypeOf(input);
    const desc = Object.getOwnPropertyDescriptor(proto, "value");
    if (desc && desc.set) {
      desc.set.call(input, val);
    } else {
      input.value = val;
    }
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  };

  const hasInputsToSet = [...inputs].some(([name]) => name !== "branch");
  if (hasInputsToSet) {
    await query(document, doc => doc.querySelector("[name^='inputs[']:not([type=hidden])"));
  }

  const setInput = async (name, value) => {
    const selector = `[class*="WorkflowDispatchMenu"] [name='inputs[${name}]'], details .workflow-dispatch [name='inputs[${name}]'], [name='inputs[${name}]']`;
    let input = document.querySelector(`${selector}:not([type=hidden])`);

    if (!input) {
      for (let i = 0; i < 30; i++) {
        await sleep(100);
        input = document.querySelector(`${selector}:not([type=hidden])`);
        if (input) break;
      }
    }

    if (!input) {
      alert(`workflow_dispatch input '${name}' does not exist`);
      return;
    }

    if (input.type === "checkbox") {
      if (!["true", "false"].includes(value)) {
        alert(`workflow_dispatch input '${name}' expects a boolean ('true' or 'false) but is set to '${value}'`);
        return;
      }
      const shouldBeChecked = value === "true";
      if (input.checked !== shouldBeChecked) {
        input.click();
      }
      if (input.checked !== shouldBeChecked) {
        input.checked = shouldBeChecked;
        input.dispatchEvent(new Event("input", { bubbles: true }));
        input.dispatchEvent(new Event("change", { bubbles: true }));
      }
    } else if (input.tagName === "SELECT") {
      input.value = value;
      input.dispatchEvent(new Event("change", { bubbles: true }));
    } else {
      setNativeValue(input, value);
    }
  };

  const branch = inputs.get("branch");
  if (branch) await setBranch(branch);

  for (const [name, value] of [...inputs].filter(([name]) => name !== "branch")) {
    await setInput(name, value);
  }

  const submitBtn =
    document.querySelector('button[data-component="Dialog.FooterButton"][type="submit"]') ||
    document.querySelector('[data-component="Dialog"] button[type="submit"]') ||
    document.querySelector("details .workflow-dispatch button[type=submit]") ||
    Array.from(document.querySelectorAll("button[type=submit]")).find(b => b.textContent.trim() === "Run workflow");

  if (submitBtn) {
    submitBtn.focus();
  }
};

const setupPrPage = async () => {
  const match = /^https:\/\/github.com\/NixOS\/nixpkgs\/pull\/(\d+)([?#].*)?$/i.exec(location.href);
  if (match === null) return;

  const pr = match[1];
  const actions = await query(document, "div[data-component=PH_Actions], .gh-header-show .gh-header-actions");

  if (actions.querySelector(".run-nixpkgs-review") === null && repo) {
    const btn = document.createElement("button");
    btn.classList.add("Button", "Button--secondary", "Button--small", "run-nixpkgs-review");
    btn.innerText = "Run nixpkgs-review";
    actions.prepend(btn);
    btn.onclick = () => {
      const params = new URLSearchParams({ ...reviewDefaults(getPrDetails(pr)), pr });
      window.open(`https://github.com/${repo}/actions/workflows/review.yml#dispatch:${params}`);
    };
  }

  const { hasLinuxRebuilds, hasDarwinRebuilds, state } = getPrDetails(pr);
  if ((!hasLinuxRebuilds && !hasDarwinRebuilds) || state == "MERGED") {
    actions.querySelector(".run-nixpkgs-review").setAttribute("aria-disabled", true);
  }

  if (actions.querySelector(".goto-pr-tracker") === null) {
    for (const { name, toUrl } of prTrackers) {
      const btn = document.createElement("button");
      btn.classList.add("Button", "Button--secondary", "Button--small", "goto-pr-tracker");
      btn.innerText = prTrackers.length === 1 ? "PR Tracker" : `PR Tracker (${name})`;
      actions.prepend(btn);
      btn.onclick = () => {
        window.open(toUrl(pr));
      };
    }
  }
};

new MutationObserver(setupPrPage).observe(document, { subtree: true, childList: true });

setupActionsPage();
setupPrPage();
