// ==UserScript==
// @name        nixpkgs-review-gha
// @match       https://github.com/*
// @homepageURL https://github.com/Defelo/nixpkgs-review-gha
// @downloadURL https://github.com/Defelo/nixpkgs-review-gha/raw/refs/heads/main/shortcut.user.js
// @run-at      document-idle
// ==/UserScript==

const user =
  document.querySelector('meta[name="user-login"]')?.content ||
  document.querySelector("header.GlobalNav button[data-login]")?.getAttribute("data-login") ||
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
  const targetsStableRelease = targetBranch?.match(/-\d\d\.\d\d$/);

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
    const elem = doc.querySelector(sel);
    if (elem !== null) return elem;
    await sleep(100);
  }
};

const queryText = sel => document.querySelector(sel)?.innerText ?? null;

// GitHub serves two PR page layouts. The overview layout puts state, author and branches in the header description.
const overviewHeader = "[data-component='PageHeader.Description']";
// Present in both layouts even when the PR has no labels.
const labelsContainer = "div.js-issue-labels, section[aria-label=Labels]";

const getLabels = () =>
  [
    ...document.querySelectorAll("div.js-issue-labels > a, section[aria-label=Labels] [data-component=LabelGroup] a"),
  ].map(x => x.innerText);
const getRebuilds = labels => ({
  hasLinuxRebuilds: !labels.some(l => /rebuild-linux: 0$/.test(l)),
  hasDarwinRebuilds: !labels.some(l => /rebuild-darwin: 0$/.test(l)),
});
const getState = () =>
  queryText(
    `div[data-component=TitleArea] div[data-component=PH_LeadingVisual] span, ${overviewHeader} [data-component=StateLabel]`,
  )
    ?.trim()
    .toUpperCase() ?? null;

// Fields whose element is missing are null.
const getPrDetails = pr => {
  const title = queryText(
    "div[data-component=TitleArea] h1 > [data-component=Text], bdi.js-issue-title.markdown-title",
  );
  // The base branch comes before the head branch.
  const targetBranch =
    queryText(
      `[data-component=PH_Title] a[data-component=BranchName], ${overviewHeader} a[data-component=BranchName]`,
    )?.replace(/^NixOS:/, "") ?? null;
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
  const labels = getLabels();
  const author =
    document
      .querySelector(`.js-discussion > :first-child a.author, ${overviewHeader} a[data-hovercard-type=user]`)
      ?.href.split("/")
      .at(-1) ?? null;
  const authoredByMe = author !== null && author === user;
  const { hasLinuxRebuilds, hasDarwinRebuilds } = getRebuilds(labels);
  const state = getState();

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

  (
    await query(
      document,
      'details > summary.btn, react-app.loaded [data-component="PH_Actions"] button[data-variant="primary"]',
    )
  ).click();
  await query(document, ':is(details .workflow-dispatch, [role="dialog"]) [name="inputs[pr]"]');

  const setBranch = async branch => {
    await query(document, 'details .workflow-dispatch, [role="dialog"]');
    const newRefPickerBtn = document.querySelector("[role=dialog] button[id^=ref-picker-]");
    if (newRefPickerBtn) {
      newRefPickerBtn.click();
      await query(document, "ul#branches");
      [...document.querySelectorAll("ul#branches > li")]
        .find(x => x.querySelector('[data-component="ActionList.Item.Label"] div div div').innerText === branch)
        .click();
      do {
        await sleep(100);
      } while (document.querySelector('[role="dialog"] [aria-busy=true]'));
    } else {
      (await query(document, "details .workflow-dispatch")).classList.add("old-branch");
      document.querySelector("details .workflow-dispatch .branch-selection > details > summary").click();
      (await query(document, `ref-selector[type=branch] button[value="${branch}"]`)).click();
      while ((await query(document, "details .workflow-dispatch")).classList.contains("old-branch")) await sleep(100);
    }
  };

  const setInput = (name, value) => {
    const selector = `:is(details .workflow-dispatch, [role="dialog"]) [name='inputs[${name}]']`;
    const input = document.querySelector(`${selector}:not([type=hidden])`);

    if (!input) {
      alert(`workflow_dispatch input '${name}' does not exist`);
      return;
    }

    if (input.type === "checkbox") {
      if (!["true", "false"].includes(value)) {
        alert(`workflow_dispatch input '${name}' expects a boolean ('true' or 'false) but is set to '${value}'`);
        return;
      }
      input.checked = value === "true";
    } else {
      input.value = value;
    }
  };

  const branch = inputs.get("branch");
  if (branch) await setBranch(branch);

  [...inputs]
    .filter(([name]) => !["branch", "missing"].includes(name))
    .forEach(([name, value]) => setInput(name, value));

  const missing = inputs.get("missing");
  if (missing) alert(`PR page elements not found: ${missing}. Check the workflow inputs before running.`);

  document.querySelector('details .workflow-dispatch button[type=submit], [role="dialog"] button[type=submit]').focus();
};

// Runs on every DOM mutation.
const setupPrPage = () => {
  const match = /^https:\/\/github.com\/NixOS\/nixpkgs\/pull\/(\d+)([?#].*)?$/i.exec(location.href);
  if (match === null) return;

  const pr = match[1];
  const actions = document.querySelector("div[data-component=PH_Actions], .gh-header-show .gh-header-actions");
  if (actions === null) return;

  if (actions.querySelector(".run-nixpkgs-review") === null && repo) {
    const btn = document.createElement("button");
    btn.classList.add("Button", "Button--secondary", "Button--small", "run-nixpkgs-review");
    btn.innerText = "Run nixpkgs-review";
    actions.prepend(btn);
    btn.onclick = () => {
      const details = getPrDetails(pr);
      const missing = ["title", "targetBranch", "author", "state"].filter(key => details[key] === null);
      if (document.querySelector(labelsContainer) === null) missing.push("labels");
      const params = new URLSearchParams({ ...reviewDefaults(details), pr });
      if (missing.length > 0) {
        console.warn(`nixpkgs-review-gha: PR page elements not found: ${missing.join(", ")}`);
        // Reported on the workflow page: an alert here can outlast the click's user activation, and the new tab is
        // then blocked as a popup.
        params.set("missing", missing.join(", "));
      }
      window.open(`https://github.com/${repo}/actions/workflows/review.yml#dispatch:${params}`);
    };
  }

  const runButton = actions.querySelector(".run-nixpkgs-review");
  if (runButton !== null) {
    const { hasLinuxRebuilds, hasDarwinRebuilds } = getRebuilds(getLabels());
    // A hint only: the button stays clickable.
    runButton.setAttribute("aria-disabled", (!hasLinuxRebuilds && !hasDarwinRebuilds) || getState() === "MERGED");
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

  // Logged out, GitHub hides the actions bar, which is empty then.
  actions.classList.remove("d-none");
};

new MutationObserver(setupPrPage).observe(document, { subtree: true, childList: true });

setupActionsPage();
setupPrPage();
