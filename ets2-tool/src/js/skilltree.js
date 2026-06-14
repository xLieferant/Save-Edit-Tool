const MAX_SKILL_LEVEL = 6;
const ADR_LEVEL_VALUES = [1, 3, 7, 15, 31, 63];

const SKILL_GROUPS = [
  {
    id: "cargo",
    labelKey: "editor.skilltree.skills.groups.cargo",
    skills: ["adr", "heavy", "fragile"],
  },
  {
    id: "operations",
    labelKey: "editor.skilltree.skills.groups.operations",
    skills: ["long_dist", "urgent", "mechanical"],
  },
];

const SKILLS = [
  {
    id: "adr",
    labelKey: "label.adr",
    descriptionKey: "editor.skilltree.skills.adr_desc",
    backendSkill: "adr",
  },
  {
    id: "long_dist",
    labelKey: "label.long_distance",
    descriptionKey: "editor.skilltree.skills.long_dist_desc",
    backendSkill: "long_dist",
  },
  {
    id: "heavy",
    labelKey: "label.heavy_cargo",
    descriptionKey: "editor.skilltree.skills.heavy_desc",
    backendSkill: "heavy",
  },
  {
    id: "fragile",
    labelKey: "label.fragile_cargo",
    descriptionKey: "editor.skilltree.skills.fragile_desc",
    backendSkill: "fragile",
  },
  {
    id: "urgent",
    labelKey: "label.just_in_time_delivery",
    descriptionKey: "editor.skilltree.skills.urgent_desc",
    backendSkill: "urgent",
  },
  {
    id: "mechanical",
    labelKey: "label.eco_driving",
    descriptionKey: "editor.skilltree.skills.mechanical_desc",
    backendSkill: "mechanical",
  },
];

const skillMap = new Map(SKILLS.map((skill) => [skill.id, skill]));

const state = {
  selectedSkillId: SKILLS[0].id,
  originalLevels: createEmptyLevels(),
  draftLevels: createEmptyLevels(),
  sourceFingerprint: null,
  isDirty: false,
  isSaving: false,
  activeRoot: null,
};

function createEmptyLevels() {
  return {
    adr: 0,
    long_dist: 0,
    heavy: 0,
    fragile: 0,
    urgent: 0,
    mechanical: 0,
  };
}

function clampLevelValue(value) {
  return Math.max(0, Math.min(MAX_SKILL_LEVEL, Number(value) || 0));
}

function adrValueToLevel(rawValue) {
  const value = Number(rawValue);
  if (!Number.isFinite(value) || value <= 0) return 0;
  const exactIndex = ADR_LEVEL_VALUES.indexOf(value);
  if (exactIndex >= 0) return exactIndex + 1;
  return clampLevelValue(Math.round(Math.log2(value + 1)));
}

function adrLevelToValue(level) {
  const normalizedLevel = clampLevelValue(level);
  if (normalizedLevel <= 0) return 0;
  return ADR_LEVEL_VALUES[normalizedLevel - 1] ?? ADR_LEVEL_VALUES[ADR_LEVEL_VALUES.length - 1];
}

function cloneLevels(levels) {
  return { ...createEmptyLevels(), ...(levels || {}) };
}

function getLevelsFromSource() {
  const data = window.currentQuicksaveData || {};
  return {
    adr: adrValueToLevel(data.adr),
    long_dist: clampLevelValue(data.long_dist),
    heavy: clampLevelValue(data.heavy),
    fragile: clampLevelValue(data.fragile),
    urgent: clampLevelValue(data.urgent),
    mechanical: clampLevelValue(data.mechanical),
  };
}

function getSourceFingerprint(levels) {
  return JSON.stringify({
    profile: window.selectedProfilePath || "",
    save: window.selectedSavePath || "",
    levels,
  });
}

function hasLoadedSaveContext() {
  return Boolean(window.selectedProfilePath && window.selectedSavePath);
}

function syncStateWithSource() {
  const sourceLevels = hasLoadedSaveContext() ? getLevelsFromSource() : createEmptyLevels();
  const nextFingerprint = getSourceFingerprint(sourceLevels);

  if (state.sourceFingerprint === nextFingerprint && state.isDirty) {
    return;
  }

  state.originalLevels = cloneLevels(sourceLevels);
  state.draftLevels = cloneLevels(sourceLevels);
  state.sourceFingerprint = nextFingerprint;
  state.isDirty = false;

  if (!skillMap.has(state.selectedSkillId)) {
    state.selectedSkillId = SKILLS[0].id;
  }
}

function areLevelsEqual(left, right) {
  return SKILLS.every((skill) => clampLevelValue(left?.[skill.id]) === clampLevelValue(right?.[skill.id]));
}

function updateDirtyState() {
  state.isDirty = !areLevelsEqual(state.draftLevels, state.originalLevels);
}

function setSelectedSkill(skillId) {
  if (!skillMap.has(skillId)) return;
  state.selectedSkillId = skillId;
}

function setSkillLevel(skillId, level) {
  if (!skillMap.has(skillId) || state.isSaving) return;
  setSelectedSkill(skillId);
  state.draftLevels[skillId] = clampLevelValue(level);
  updateDirtyState();
}

function resetSelectedSkill() {
  const skillId = state.selectedSkillId;
  if (!skillMap.has(skillId) || state.isSaving) return;
  state.draftLevels[skillId] = state.originalLevels[skillId];
  updateDirtyState();
}

function resetDraft() {
  if (state.isSaving) return;
  state.draftLevels = cloneLevels(state.originalLevels);
  updateDirtyState();
}

function setAllSkills(level) {
  if (state.isSaving) return;
  const normalizedLevel = clampLevelValue(level);
  for (const skill of SKILLS) {
    state.draftLevels[skill.id] = normalizedLevel;
  }
  updateDirtyState();
}

function getTotals() {
  const assigned = SKILLS.reduce((sum, skill) => sum + clampLevelValue(state.draftLevels[skill.id]), 0);
  const max = SKILLS.length * MAX_SKILL_LEVEL;
  return {
    assigned,
    max,
    remaining: Math.max(0, max - assigned),
    percent: max > 0 ? Math.round((assigned / max) * 100) : 0,
  };
}

function getPeakSkill() {
  let peakSkill = null;
  let peakLevel = 0;

  for (const skill of SKILLS) {
    const level = clampLevelValue(state.draftLevels[skill.id]);
    if (level > peakLevel) {
      peakLevel = level;
      peakSkill = skill;
    }
  }

  return { peakSkill, peakLevel };
}

function getStatusKey() {
  if (!hasLoadedSaveContext()) return "editor.skilltree.status_requires_save";
  if (state.isSaving) return "editor.skilltree.status_saving";
  if (state.isDirty) return "editor.skilltree.status_unsaved";
  const { assigned, max } = getTotals();
  if (assigned === 0) return "editor.skilltree.status_empty";
  if (assigned === max) return "editor.skilltree.status_maxed";
  return "editor.skilltree.status_ready";
}

function getSkillStatusKey(level) {
  if (level <= 0) return "editor.skilltree.level_zero";
  if (level >= MAX_SKILL_LEVEL) return "editor.skilltree.level_max";
  return "editor.skilltree.level_value";
}

async function getSkilltreeCopy() {
  const totals = getTotals();
  const statusText = await window.t(getStatusKey());
  const peak = getPeakSkill();
  const peakText = peak.peakSkill
    ? await window.t("editor.skilltree.peak_value", {
      skill: await window.t(peak.peakSkill.labelKey),
      level: peak.peakLevel,
    })
    : await window.t("editor.skilltree.peak_none");

  return {
    title: await window.t("editor.skilltree.title"),
    subtitle: await window.t("editor.skilltree.subtitle"),
    note: await window.t("editor.skilltree.note"),
    emptyTitle: await window.t("editor.skilltree.empty_title"),
    emptyBody: await window.t("editor.skilltree.empty_body"),
    progressLabel: await window.t("editor.skilltree.summary_progress"),
    assignedLabel: await window.t("editor.skilltree.summary_assigned"),
    peakLabel: await window.t("editor.skilltree.summary_peak"),
    statusLabel: await window.t("editor.skilltree.summary_status"),
    openLabel: await window.t("editor.skilltree.summary_open"),
    detailTitle: await window.t("editor.skilltree.detail.title"),
    detailHint: await window.t("editor.skilltree.detail.hint"),
    detailCurrent: await window.t("editor.skilltree.detail.current"),
    detailDescription: await window.t("editor.skilltree.detail.description"),
    detailControls: await window.t("editor.skilltree.detail.controls"),
    detailReset: await window.t("editor.skilltree.detail.reset"),
    detailFallback: await window.t("editor.skilltree.detail.selected_fallback"),
    apply: await window.t("editor.skilltree.actions.apply"),
    reset: await window.t("editor.skilltree.actions.reset"),
    clear: await window.t("editor.skilltree.actions.clear"),
    max: await window.t("editor.skilltree.actions.max"),
    progressValue: await window.t("editor.skilltree.progress_value", { value: totals.percent }),
    assignedValue: await window.t("editor.skilltree.assigned_value", {
      value: totals.assigned,
      max: totals.max,
    }),
    remainingValue: await window.t("editor.skilltree.remaining_value", { value: totals.remaining }),
    peakValue: peakText,
    statusValue: statusText,
  };
}

async function renderSkillCard(skill, groupLabel) {
  const level = clampLevelValue(state.draftLevels[skill.id]);
  const progressPercent = Math.round((level / MAX_SKILL_LEVEL) * 100);
  const statusKey = getSkillStatusKey(level);
  const label = await window.t(skill.labelKey);
  const description = await window.t(skill.descriptionKey);
  const status = statusKey === "editor.skilltree.level_value"
    ? await window.t(statusKey, { level, max: MAX_SKILL_LEVEL })
    : await window.t(statusKey);
  const selectedClass = state.selectedSkillId === skill.id ? " is-selected" : "";
  const maxedClass = level >= MAX_SKILL_LEVEL ? " is-maxed" : "";
  const emptyClass = level === 0 ? " is-empty" : "";

  const nodes = [];
  for (let currentLevel = 0; currentLevel <= MAX_SKILL_LEVEL; currentLevel += 1) {
    const isActive = currentLevel !== 0 && currentLevel <= level;
    const isCurrent = currentLevel === level;
    const isZeroNode = currentLevel === 0;
    nodes.push(`
      <button
        type="button"
        class="skilltree-node${isActive ? " is-active" : ""}${isCurrent ? " is-current" : ""}${isZeroNode ? " is-zero" : ""}"
        data-skill-node="${skill.id}"
        data-level="${currentLevel}"
        aria-pressed="${isCurrent ? "true" : "false"}"
      >
        <span>${currentLevel}</span>
      </button>
    `);
  }

  return `
    <article
      class="skilltree-card${selectedClass}${maxedClass}${emptyClass}"
      data-skill-card="${skill.id}"
      tabindex="0"
      role="button"
      aria-label="${label}"
    >
      <div class="skilltree-card-head">
        <span class="skilltree-card-group">${groupLabel}</span>
        <span class="skilltree-card-badge">${status}</span>
      </div>
      <div class="skilltree-card-copy">
        <h3>${label}</h3>
        <p>${description}</p>
      </div>
      <div class="skilltree-progress-shell" aria-hidden="true">
        <div class="skilltree-progress-bar">
          <span class="skilltree-progress-fill" style="width: ${progressPercent}%;"></span>
        </div>
        <strong class="skilltree-progress-value">${level}/${MAX_SKILL_LEVEL}</strong>
      </div>
      <div class="skilltree-node-track" role="group" aria-label="${label}">
        ${nodes.join("")}
      </div>
    </article>
  `;
}

async function renderSkillGroups() {
  const groups = [];

  for (const group of SKILL_GROUPS) {
    const groupLabel = await window.t(group.labelKey);
    const cards = await Promise.all(
      group.skills.map((skillId) => renderSkillCard(skillMap.get(skillId), groupLabel))
    );

    groups.push(`
      <section class="skilltree-group" aria-label="${groupLabel}">
        <div class="skilltree-group-head">
          <span class="skilltree-group-label">${groupLabel}</span>
        </div>
        <div class="skilltree-group-grid">
          ${cards.join("")}
        </div>
      </section>
    `);
  }

  return groups.join("");
}

async function renderDetailPanel(copy) {
  const skill = skillMap.get(state.selectedSkillId);
  if (!skill) {
    return `
      <aside class="skilltree-detail skilltree-detail--empty">
        <h3>${copy.detailTitle}</h3>
        <p>${copy.detailFallback}</p>
      </aside>
    `;
  }

  const level = clampLevelValue(state.draftLevels[skill.id]);
  const label = await window.t(skill.labelKey);
  const description = await window.t(skill.descriptionKey);
  const statusKey = getSkillStatusKey(level);
  const status = statusKey === "editor.skilltree.level_value"
    ? await window.t(statusKey, { level, max: MAX_SKILL_LEVEL })
    : await window.t(statusKey);

  const controlButtons = [];
  for (let currentLevel = 0; currentLevel <= MAX_SKILL_LEVEL; currentLevel += 1) {
    controlButtons.push(`
      <button
        type="button"
        class="skilltree-level-btn${currentLevel === level ? " is-active" : ""}"
        data-skill-level="${skill.id}"
        data-level="${currentLevel}"
        aria-pressed="${currentLevel === level ? "true" : "false"}"
      >
        ${currentLevel}
      </button>
    `);
  }

  return `
    <aside class="skilltree-detail">
      <div class="skilltree-detail-top">
        <span class="skilltree-detail-kicker">${copy.detailTitle}</span>
        <h3>${label}</h3>
        <span class="skilltree-detail-status">${status}</span>
      </div>

      <div class="skilltree-detail-grid">
        <article class="skilltree-detail-card">
          <span>${copy.detailCurrent}</span>
          <strong>${level}/${MAX_SKILL_LEVEL}</strong>
        </article>
        <article class="skilltree-detail-card">
          <span>${copy.detailDescription}</span>
          <p>${description}</p>
        </article>
      </div>

      <div class="skilltree-stepper" data-skill-stepper="${skill.id}">
        <button type="button" class="skilltree-stepper-btn" data-skill-adjust="${skill.id}" data-delta="-1" ${level <= 0 ? "disabled" : ""}>-</button>
        <div class="skilltree-stepper-value">
          <span>${copy.detailControls}</span>
          <strong>${await window.t("editor.skilltree.level_value", { level, max: MAX_SKILL_LEVEL })}</strong>
        </div>
        <button type="button" class="skilltree-stepper-btn" data-skill-adjust="${skill.id}" data-delta="1" ${level >= MAX_SKILL_LEVEL ? "disabled" : ""}>+</button>
      </div>

      <div class="skilltree-level-grid" role="group" aria-label="${label}">
        ${controlButtons.join("")}
      </div>

      <div class="skilltree-detail-actions">
        <button type="button" class="secondary-action skilltree-detail-reset" data-skill-reset="${skill.id}">
          ${copy.detailReset}
        </button>
      </div>

      <p class="skilltree-detail-hint">${copy.detailHint}</p>
    </aside>
  `;
}

async function renderSkilltree(root) {
  state.activeRoot = root;
  syncStateWithSource();

  const copy = await getSkilltreeCopy();
  const totals = getTotals();
  const hasContext = hasLoadedSaveContext();
  const statusState = state.isSaving
    ? "saving"
    : state.isDirty
      ? "dirty"
      : totals.assigned === totals.max && totals.max > 0
        ? "maxed"
        : "ready";

  if (!hasContext) {
    root.innerHTML = `
      <section class="skilltree-surface skilltree-surface--empty">
        <div class="skilltree-head">
          <div class="skilltree-heading">
            <span class="eyebrow">${copy.title}</span>
            <h3>${copy.title}</h3>
            <p>${copy.subtitle}</p>
          </div>
          <span class="skilltree-status-pill" data-state="idle">${copy.statusValue}</span>
        </div>
        <div class="skilltree-empty-state">
          <strong>${copy.emptyTitle}</strong>
          <p>${copy.emptyBody}</p>
        </div>
      </section>
    `;
    return;
  }

  root.innerHTML = `
    <section class="skilltree-surface">
      <div class="skilltree-head">
        <div class="skilltree-heading">
          <span class="eyebrow">${copy.title}</span>
          <h3>${copy.title}</h3>
          <p>${copy.subtitle}</p>
        </div>
        <span class="skilltree-status-pill" data-state="${statusState}">${copy.statusValue}</span>
      </div>

      <div class="skilltree-summary-grid">
        <article class="skilltree-summary-card skilltree-summary-card--progress">
          <span>${copy.progressLabel}</span>
          <strong>${copy.progressValue}</strong>
          <div class="skilltree-summary-progress" aria-hidden="true">
            <span style="width: ${totals.percent}%;"></span>
          </div>
        </article>
        <article class="skilltree-summary-card">
          <span>${copy.assignedLabel}</span>
          <strong>${copy.assignedValue}</strong>
          <small>${copy.openLabel}: ${copy.remainingValue}</small>
        </article>
        <article class="skilltree-summary-card">
          <span>${copy.peakLabel}</span>
          <strong>${copy.peakValue}</strong>
          <small>${copy.note}</small>
        </article>
        <article class="skilltree-summary-card">
          <span>${copy.statusLabel}</span>
          <strong>${copy.statusValue}</strong>
          <small>${copy.openLabel}: ${copy.remainingValue}</small>
        </article>
      </div>

      <div class="skilltree-workspace">
        <div class="skilltree-canvas">
          ${await renderSkillGroups()}
        </div>
        ${await renderDetailPanel(copy)}
      </div>

      <div class="skilltree-actions">
        <button type="button" class="table-action skilltree-apply" data-skilltree-action="apply" ${!state.isDirty || state.isSaving ? "disabled" : ""}>
          ${copy.apply}
        </button>
        <button type="button" class="secondary-action" data-skilltree-action="reset" ${!state.isDirty || state.isSaving ? "disabled" : ""}>
          ${copy.reset}
        </button>
        <button type="button" class="secondary-action" data-skilltree-action="clear" ${state.isSaving ? "disabled" : ""}>
          ${copy.clear}
        </button>
        <button type="button" class="secondary-action" data-skilltree-action="max" ${state.isSaving ? "disabled" : ""}>
          ${copy.max}
        </button>
      </div>

      <p class="skilltree-note">${copy.note}</p>
    </section>
  `;
}

async function applyDraftToSave(root) {
  if (!state.isDirty || state.isSaving || !hasLoadedSaveContext()) return;

  state.isSaving = true;
  await renderSkilltree(root);

  try {
    for (const skill of SKILLS) {
      const level = clampLevelValue(state.draftLevels[skill.id]);
      const value = skill.id === "adr" ? adrLevelToValue(level) : level;
      await window.invoke("edit_skill_value", {
        skill: skill.backendSkill,
        value,
      });
    }

    await window.loadQuicksave?.();
    syncStateWithSource();
    await renderSkilltree(root);
    await window.showToast("toasts.change_skill_points_success", "success");
  } catch (error) {
    console.error("Skilltree apply error:", error);
    await window.showToast("toasts.change_skill_points_error", "error");
  } finally {
    state.isSaving = false;
    await renderSkilltree(root);
  }
}

async function handleAction(root, action) {
  if (state.isSaving) return;

  if (action === "reset") {
    resetDraft();
    await renderSkilltree(root);
    return;
  }

  if (action === "clear") {
    setAllSkills(0);
    await renderSkilltree(root);
    return;
  }

  if (action === "max") {
    setAllSkills(MAX_SKILL_LEVEL);
    await renderSkilltree(root);
    return;
  }

  if (action === "apply") {
    await applyDraftToSave(root);
  }
}

async function handleClick(event) {
  const root = event.currentTarget;

  const treeAction = event.target.closest("[data-skilltree-action]");
  if (treeAction) {
    await handleAction(root, treeAction.dataset.skilltreeAction);
    return;
  }

  const node = event.target.closest("[data-skill-node]");
  if (node) {
    setSkillLevel(node.dataset.skillNode, node.dataset.level);
    await renderSkilltree(root);
    return;
  }

  const levelButton = event.target.closest("[data-skill-level]");
  if (levelButton) {
    setSkillLevel(levelButton.dataset.skillLevel, levelButton.dataset.level);
    await renderSkilltree(root);
    return;
  }

  const adjustButton = event.target.closest("[data-skill-adjust]");
  if (adjustButton) {
    const skillId = adjustButton.dataset.skillAdjust;
    const delta = Number(adjustButton.dataset.delta || 0);
    setSkillLevel(skillId, clampLevelValue(state.draftLevels[skillId] + delta));
    await renderSkilltree(root);
    return;
  }

  const resetButton = event.target.closest("[data-skill-reset]");
  if (resetButton) {
    setSelectedSkill(resetButton.dataset.skillReset);
    resetSelectedSkill();
    await renderSkilltree(root);
    return;
  }

  const card = event.target.closest("[data-skill-card]");
  if (card) {
    setSelectedSkill(card.dataset.skillCard);
    await renderSkilltree(root);
  }
}

async function handleKeydown(event) {
  const root = event.currentTarget;

  const card = event.target.closest("[data-skill-card]");
  if (card && (event.key === "Enter" || event.key === " ")) {
    event.preventDefault();
    setSelectedSkill(card.dataset.skillCard);
    await renderSkilltree(root);
    return;
  }

  const node = event.target.closest("[data-skill-node], [data-skill-level]");
  if (!node) return;

  const skillId = node.dataset.skillNode || node.dataset.skillLevel;
  const currentLevel = clampLevelValue(node.dataset.level);

  if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
    event.preventDefault();
    setSkillLevel(skillId, currentLevel - 1);
    await renderSkilltree(root);
    const nextButton = root.querySelector(`[data-skill-node="${skillId}"][data-level="${Math.max(0, currentLevel - 1)}"], [data-skill-level="${skillId}"][data-level="${Math.max(0, currentLevel - 1)}"]`);
    nextButton?.focus();
  }

  if (event.key === "ArrowRight" || event.key === "ArrowUp") {
    event.preventDefault();
    setSkillLevel(skillId, currentLevel + 1);
    await renderSkilltree(root);
    const nextButton = root.querySelector(`[data-skill-node="${skillId}"][data-level="${Math.min(MAX_SKILL_LEVEL, currentLevel + 1)}"], [data-skill-level="${skillId}"][data-level="${Math.min(MAX_SKILL_LEVEL, currentLevel + 1)}"]`);
    nextButton?.focus();
  }
}

function bindEvents(root) {
  if (root.dataset.skilltreeBound === "true") return;
  root.dataset.skilltreeBound = "true";
  root.addEventListener("click", (event) => {
    void handleClick(event);
  });
  root.addEventListener("keydown", (event) => {
    void handleKeydown(event);
  });
}

export async function mountSkilltreeEditor(root) {
  if (!root) return;
  bindEvents(root);
  await renderSkilltree(root);
}
