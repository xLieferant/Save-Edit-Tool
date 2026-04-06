import { attachI18nToWindow, t, translateDocument } from "../shared/i18n.js";
import { hasTauri, invoke, listen, safeInvoke } from "../shared/runtime.js";

const state = {
  overview: null,
  status: null,
  jobLog: [],
  jobStats: null,
  source: "mock",
  vtcContext: null,
  authUser: null,
  userProfile: null,
  userSettings: null,
  companyOverview: null,
  companyMembers: [],
  companySettings: null,
  careerSettings: null,
  roles: [],
  dispatcher: {
    overview: null,
    generation: null,
    marketJobs: [],
    selectedJobId: null,
    selectedJob: null,
    jobLink: null,
    activeJobs: [],
    history: null,
    contacts: [],
    offers: [],
    liveProgress: {},
    lastError: {},
    writeReport: {},
    snapshotDiagnostics: null,
    actionBusy: false,
    tab: "market",
  },
};

const FALLBACK_ROLES = [
  "owner",
  "ceo",
  "manager",
  "dispatcher",
  "driver",
  "trainee",
  "recruiter",
  "mechanic",
];

const uiText = {
  online: "Online",
  offline: "Offline",
  noData: "-",
  sourceLive: "Live",
  sourceMock: "Mock",
  companyLiveDesc: "-",
  companyMockDesc: "-",
  genericSaved: "Saved",
  errorPrefix: "Error",
  dispatcherAccept: "Accept",
  dispatcherReject: "Reject",
  dispatcherCancel: "Cancel",
  dispatcherBadgeGenerated: "Generated",
  dispatcherBadgeOffer: "Offer",
  dispatcherBadgeContract: "Contract",
  dispatcherBadgeImported: "Imported",
  dispatcherBadgeActive: "Active",
  dispatcherBadgePending: "Pending",
  dispatcherBadgeRequiresLoad: "Requires Load",
  dispatcherBadgeSynced: "Synced to ETS2",
  dispatcherBadgeCompleted: "Completed",
  dispatcherBadgeError: "Error",
  dispatcherBadgePendingRoute: "Pending Route",
  dispatcherBadgeSaveLinked: "Save-linked",
  dispatcherNoSaveLink: "No save linked",
  dispatcherUnsynced: "Not synced",
  dispatcherExpired: "Expired",
  dispatcherAssigned: "Assigned to Save",
  dispatcherInjectActiveSave: "Inject into Active Save",
  dispatcherAlreadyInjected: "Already Injected",
  dispatcherPrepareLink: "Prepare ETS Link",
  dispatcherRetryPrepareLink: "Retry Prepare ETS Link",
  dispatcherWriteQuicksave: "Write to Quicksave",
  dispatcherIntervalLabel: "Interval",
  dispatcherPoolLabel: "Pool",
  dispatcherOpenLabel: "Open",
  vtcContextNoProfile: "No profile linked",
  vtcContextNoSave: "No save selected",
  vtcContextSessionLinked: "Session linked",
  vtcContextSessionInferred: "Session inferred",
  vtcContextSessionProfileOnly: "Profile detected, save pending",
  vtcContextSessionMissing: "No live save context",
  userMenuAccount: "Account",
  userMenuLogin: "Login",
  userMenuNotLoggedIn: "Not logged in",
};

function formatNumber(value, digits = 0) {
  return Number(value || 0).toLocaleString(undefined, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

function formatCurrency(value) {
  return `EUR ${formatNumber(value || 0, 0)}`;
}

function formatDistance(value) {
  return `${formatNumber(value || 0, 1)} km`;
}

function formatPercent(value) {
  return `${formatNumber(Number(value || 0) * 100, 0)}%`;
}

function formatDate(value) {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);
  return date.toLocaleString();
}

function formatMinutes(value) {
  const totalMinutes = Math.max(0, Number(value || 0));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = Math.round(totalMinutes % 60);
  if (hours <= 0) return `${minutes} min`;
  if (minutes <= 0) return `${hours} h`;
  return `${hours} h ${minutes} min`;
}

function formatRelativeTime(value) {
  if (!value) return "-";
  const target = new Date(value);
  if (Number.isNaN(target.getTime())) return String(value);
  const diffMs = target.getTime() - Date.now();
  if (diffMs <= 0) return uiText.dispatcherExpired;
  const totalMinutes = Math.ceil(diffMs / 60000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours <= 0) return `${minutes} min`;
  if (minutes <= 0) return `${hours} h`;
  return `${hours} h ${minutes} min`;
}

function formatPathReference(value) {
  if (!value) return uiText.dispatcherNoSaveLink;
  const normalized = String(value).replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  if (parts.length <= 2) return normalized;
  return parts.slice(-2).join("/");
}

function formatVtcPathReference(value, fallback) {
  if (!value) return fallback;
  return formatPathReference(value);
}

function sessionStatusLabel(status) {
  switch (String(status || "").toLowerCase()) {
    case "linked":
      return uiText.vtcContextSessionLinked;
    case "inferred":
      return uiText.vtcContextSessionInferred;
    case "profile_only":
      return uiText.vtcContextSessionProfileOnly;
    default:
      return uiText.vtcContextSessionMissing;
  }
}

function hasActiveSaveContext() {
  const context = state.vtcContext || {};
  return Boolean(context.saveReference || context.saveSessionId);
}

function currentDispatcherJob() {
  return state.dispatcher.selectedJob?.job || state.dispatcher.selectedJob || null;
}

function dispatcherActionAvailability(job, jobLink) {
  const status = String(job?.status || "").toLowerCase();
  const rawLinkStatus = String(jobLink?.status || job?.ets2JobLinkStatus || "").toLowerCase();
  const saveLinked = Boolean(job?.linkedToActiveSave || job?.saveReference);
  const activeSaveReady = hasActiveSaveContext();

  const canAccept = Boolean(job?.id) && status === "open";
  const canAssign = Boolean(job?.id) && activeSaveReady && ["open", "accepted", "failed"].includes(status);
  const canPrepare = Boolean(job?.id) && saveLinked && (["assigned_to_save", "failed"].includes(status) || rawLinkStatus === "error");
  const canWrite = Boolean(job?.id) && Boolean(jobLink?.linkId) && ["prepared", "written", "pending"].includes(rawLinkStatus);
  const alreadyInjected = ["injected", "completed"].includes(status)
    || ["requires_load", "synced", "completed", "synced_to_ets2"].includes(rawLinkStatus);
  const injectableStatuses = ["open", "accepted", "failed", "assigned_to_save", "prepared"];
  const canInject = Boolean(job?.id)
    && activeSaveReady
    && injectableStatuses.includes(status)
    && !alreadyInjected;
  const canMarkSynced = Boolean(job?.id) && (
    status === "injected"
      || ["written", "requires_load", "synced", "completed", "synced_to_ets2"].includes(rawLinkStatus)
  );

  return { canAccept, canAssign, canPrepare, canWrite, canInject, canMarkSynced };
}

function formatDispatcherWriteReport(report) {
  if (!report) return "-";
  if (report.error) {
    const lines = [
      `dispatcher_job_id: ${report.jobId || "-"}`,
      `write_result: failed`,
      `error: ${report.error}`,
    ];
    if (report.diagnostics) {
      for (const [key, value] of Object.entries(report.diagnostics)) {
        lines.push(`${key}: ${value}`);
      }
    }
    return lines.join("\n");
  }
  const lines = [
    `dispatcher_job_id: ${report.jobId || "-"}`,
    `save_reference: ${report.saveReference || "-"}`,
    `quicksave_reference: ${report.quicksaveReference || "-"}`,
    `save_session_id: ${report.saveSessionId || "-"}`,
    `assign_result: ${report.assignResult || "-"}`,
    `prepare_result: ${report.prepareResult || "-"}`,
    `write_result: ${report.writeResult || "-"}`,
    `save_path: ${report.savePath || "-"}`,
    `backup_path: ${report.backupPath || "-"}`,
    `company_pointer: ${report.companyPointer || "-"}`,
    `offer_pointer: ${report.offerPointer || "-"}`,
    `job_offer_data_pointer: ${report.jobOfferDataPointer || "-"}`,
    `origin: ${report.origin || "-"}`,
    `destination: ${report.destination || "-"}`,
    `target_company: ${report.targetCompany || "-"}`,
    `cargo_token: ${report.cargoToken || "-"}`,
    `shortest_distance_km: ${report.shortestDistanceKm ?? "-"}`,
    `expiration_time: ${report.expirationTime ?? "-"}`,
    `reward: ${report.reward ?? "-"}`,
    `write_mode: ${report.writeMode || "overwrite_existing_offer"}`,
    `before_sha256: ${report.beforeSha256 || "-"}`,
    `after_sha256: ${report.afterSha256 || "-"}`,
    `job_info_updated: ${report.jobInfoUpdated == null ? "-" : String(report.jobInfoUpdated)}`,
    `final_dispatcher_status: ${report.finalDispatcherStatus || "-"}`,
    `final_link_status: ${report.finalLinkStatus || "-"}`,
  ];
  return lines.join("\n");
}

function setCellHtml(id, value) {
  const element = document.getElementById(id);
  if (!element) return;
  element.innerHTML = value ?? "";
}

function setCellText(id, value) {
  const element = document.getElementById(id);
  if (!element) return;
  element.textContent = value ?? uiText.noData;
}

function setInputValue(id, value) {
  const element = document.getElementById(id);
  if (!element) return;
  element.value = value ?? "";
}

function setInputChecked(id, value) {
  const element = document.getElementById(id);
  if (!element) return;
  element.checked = Boolean(value);
}

function getInputValue(id) {
  return document.getElementById(id)?.value ?? "";
}

function getInputChecked(id) {
  return Boolean(document.getElementById(id)?.checked);
}

function showToast(message, type = "info") {
  const toast = document.createElement("div");
  toast.className = "career-toast";
  toast.dataset.type = type;
  toast.textContent = message;
  document.body.appendChild(toast);
  setTimeout(() => {
    toast.remove();
  }, 3500);
}

function normalizeErrorCode(rawError) {
  const raw =
    rawError && typeof rawError === "object"
      ? `${String(rawError.code || "")} ${String(rawError.message || "")}`
      : String(rawError || "");
  const knownCodes = [
    "username_already_taken",
    "username_change_cooldown_active",
    "user_not_found",
    "company_not_found",
    "user_already_in_company",
    "member_not_found",
    "invalid_role",
    "not_allowed",
    "company_name_already_taken",
    "invalid_game",
    "invalid_username",
    "dispatcher_job_not_open",
    "dispatcher_job_expired",
    "dispatcher_active_job_exists",
    "dispatcher_equipment_requirement_not_met",
    "dispatcher_reputation_requirement_not_met",
    "dispatcher_offer_company_required",
    "dispatcher_offer_not_cancellable",
    "dispatcher_offer_not_countered",
    "dispatcher_save_context_missing",
    "dispatcher_job_not_generated",
    "no_active_save",
    "dispatcher_job_not_found",
    "job_already_assigned",
    "invalid_job_status",
    "save_context_unavailable",
    "profile_not_found",
    "save_not_found",
    "decode_failed",
    "company_not_found_in_save",
    "company_has_no_job_offers",
    "invalid_token",
    "write_failed",
    "backup_failed",
    "lock_timeout",
    "telemetry_unavailable",
    "job_link_conflict",
    "rollback_failed",
    "steam_cloud_enabled",
  ];

  for (const code of knownCodes) {
    if (raw.includes(code)) return code;
  }

  return null;
}

async function resolveErrorMessage(rawError) {
  const code = normalizeErrorCode(rawError);
  if (!code) {
    if (rawError && typeof rawError === "object" && rawError.message) {
      return `${uiText.errorPrefix}: ${rawError.message}`;
    }
    return `${uiText.errorPrefix}: ${String(rawError || "unknown")}`;
  }

  const translationKey = `career_mode.errors.${code}`;
  const translated = await t(translationKey);
  if (translated === translationKey) {
    if (rawError && typeof rawError === "object" && rawError.message) {
      return `${uiText.errorPrefix}: ${rawError.message}`;
    }
    return `${uiText.errorPrefix}: ${code}`;
  }
  return translated;
}

async function invokeStrict(command, args = {}) {
  if (!hasTauri) {
    throw new Error("not_allowed");
  }
  return invoke(command, args);
}

function buildFallbackOverview() {
  const now = Date.now();
  return {
    economy: {
      companyName: "Alex Logistics",
      insuranceDailyCost: 360,
    },
    bank: {
      cashBalance: 485000,
    },
    reputation: {
      label: "Trusted",
      level: 8,
      xpPoints: 12750,
    },
    employeeOverview: {
      onDuty: 5,
      resting: 2,
    },
    recentJobs: [
      {
        startedAtUtc: new Date(now - 1000 * 60 * 60 * 5).toISOString(),
        originCity: "Cologne",
        destinationCity: "Frankfurt",
        cargo: "Machinery",
        plannedDistanceKm: 188,
        income: 7600,
        status: "completed",
      },
      {
        startedAtUtc: new Date(now - 1000 * 60 * 60 * 16).toISOString(),
        originCity: "Munich",
        destinationCity: "Vienna",
        cargo: "Food",
        plannedDistanceKm: 435,
        income: 15200,
        status: "completed",
      },
    ],
    fleet: [
      {
        kind: "truck",
        brand: "Scania",
        model: "S 730",
        conditionPercent: 94,
        serviceDueKm: 14200,
      },
      {
        kind: "trailer",
        brand: "Krone",
        model: "Cool Liner",
        conditionPercent: 89,
        serviceDueKm: 18750,
      },
    ],
    dashboard: {
      liveIncome: 33800,
      fuelCost: 5200,
      repairCost: 1400,
      tollCost: 2200,
      driversOnline: 5,
      driversResting: 2,
    },
    statistics: {
      completedTrips: 46,
      totalKilometers: 37892,
      totalIncome: 482300,
      companyValue: 967300,
      averageSpeed: 71.3,
      speedingEvents: 5,
    },
    jobStats: {
      totalJobs: 46,
      totalIncome: 482300,
      averageDistanceKm: 381.2,
      successRate: 0.93,
    },
    activeJob: {
      jobId: "JOB-001",
      originCity: "Berlin",
      destinationCity: "Prague",
      cargo: "Medical Supplies",
      income: 18400,
      remainingTimeMin: 318,
    },
  };
}

function setTableRows(bodyId, rows, emptyMessage, colCount) {
  const body = document.getElementById(bodyId);
  if (!body) return;

  if (!Array.isArray(rows) || rows.length === 0) {
    body.innerHTML = `<tr><td colspan="${colCount}">${emptyMessage}</td></tr>`;
    return;
  }

  body.innerHTML = rows.join("");
}

function roleOptions() {
  if (state.roles.length) {
    return state.roles;
  }
  return FALLBACK_ROLES.map((roleKey, index) => ({
    roleKey,
    roleLabel: roleKey,
    sortOrder: index + 1,
  }));
}

function renderRoleOptions(selectId, selected) {
  const select = document.getElementById(selectId);
  if (!select) return;
  const options = roleOptions();
  select.innerHTML = options
    .map((role) => `<option value="${role.roleKey}">${role.roleLabel}</option>`)
    .join("");

  if (selected) {
    select.value = selected;
  }
}

function renderDashboard() {
  const overview = state.overview;
  if (!overview) return;

  const companyName = state.companyOverview?.name || overview.economy?.companyName || uiText.noData;
  const balance = overview.bank?.cashBalance || 0;
  const level = overview.reputation?.level || 0;
  const xp = overview.reputation?.xpPoints || 0;
  const onDuty = overview.employeeOverview?.onDuty ?? 0;
  const resting = overview.employeeOverview?.resting ?? 0;
  const lastTrip = overview.recentJobs?.[0];

  setCellText("cmCompanyName", companyName);
  setCellText("cmBalance", formatCurrency(balance));
  setCellText("cmLevel", `L${level}`);
  setCellText("cmXp", formatNumber(xp, 0));
  setCellText("cmDriverStatus", `${onDuty} / ${resting}`);
  setCellText(
    "cmLastTrip",
    lastTrip ? `${lastTrip.originCity || "-"} -> ${lastTrip.destinationCity || "-"}` : uiText.noData
  );

  const overviewList = document.getElementById("cmQuickOverviewList");
  if (overviewList) {
    const lines = [
      `${companyName}`,
      `${formatCurrency(balance)}`,
      `${formatNumber(overview.jobStats?.totalJobs || state.jobStats?.totalJobs || 0)} Jobs`,
      `${Math.round(Number(overview.jobStats?.successRate || state.jobStats?.successRate || 0) * 100)}% Success`,
    ];
    overviewList.innerHTML = lines.map((line) => `<li>${line}</li>`).join("");
  }

  const latestRows = (overview.recentJobs || []).slice(0, 6).map((job) => {
    const route = `${job.originCity || "-"} -> ${job.destinationCity || "-"}`;
    const resultStatus = job.resultStatus || job.status || "-";
    return `
      <tr>
        <td>${job.cargo || "-"}</td>
        <td>${route}</td>
        <td>${formatCurrency(job.vtcExpectedIncome ?? job.income ?? 0)}</td>
        <td>${dispatcherStatusMarkup(resultStatus)}</td>
      </tr>
    `;
  });

  setTableRows("cmLatestJobsBody", latestRows, uiText.noData, 4);
}

function renderMembers() {
  const rows = (state.companyMembers || []).map((member) => {
    const optionsHtml = roleOptions()
      .map((role) => {
        const selected = role.roleKey === member.roleKey ? "selected" : "";
        return `<option value="${role.roleKey}" ${selected}>${role.roleLabel}</option>`;
      })
      .join("");

    return `
      <tr>
        <td>${member.username || "-"}</td>
        <td>
          <select class="career-member-role-select" data-member-role-select="${member.id}">
            ${optionsHtml}
          </select>
        </td>
        <td>${formatDate(member.joinedAt)}</td>
        <td>
          <button class="career-mini-btn" data-member-role-apply="${member.id}">${uiText.genericSaved}</button>
        </td>
      </tr>
    `;
  });

  setTableRows("cmMembersBody", rows, uiText.noData, 4);
}

function renderOrders() {
  const overview = state.overview || {};
  const activeJob = overview.activeJob || overview.currentJob || null;
  const activeRows = [];

  if (activeJob) {
    const route = `${activeJob.originCity || activeJob.origin || "-"} -> ${activeJob.destinationCity || activeJob.destination || "-"}`;
    const remaining = activeJob.remainingTimeMin != null
      ? `${formatNumber(activeJob.remainingTimeMin, 0)} min`
      : `${formatNumber(activeJob.deliveryEtaHours || 0, 0)} h`;
    const plannedDistance = activeJob.plannedDistanceKm || activeJob.planned_distance_km || activeJob.distanceKm || 0;
    const ingameIncome = activeJob.ingameIncome ?? activeJob.income ?? activeJob.payout ?? 0;
    const expectedIncome = activeJob.vtcExpectedIncome ?? activeJob.vtc_expected_income ?? ingameIncome;
    const resultStatus = activeJob.resultStatus || activeJob.status || "active";

    activeRows.push(`
      <tr>
        <td>${activeJob.jobId || activeJob.id || "-"}</td>
        <td>${route}</td>
        <td>${activeJob.cargo || "-"}</td>
        <td>${formatDistance(plannedDistance)}</td>
        <td>${formatCurrency(expectedIncome)}</td>
        <td>${formatCurrency(ingameIncome)}</td>
        <td>
          <div class="dispatcher-row-primary">${dispatcherStatusMarkup(resultStatus)}</div>
          <div class="dispatcher-row-secondary">${remaining}</div>
        </td>
      </tr>
    `);
  }

  setTableRows("cmActiveOrdersBody", activeRows, uiText.noData, 7);

  const historyRows = (state.jobLog || []).slice(0, 20).map((job) => {
    const route = `${job.originCity || "-"} -> ${job.destinationCity || "-"}`;
    const plannedDistance = job.plannedDistanceKm || job.planned_distance_km || 0;
    const ingameIncome = job.ingameIncome ?? job.income ?? 0;
    const expectedIncome = job.vtcExpectedIncome ?? job.vtc_expected_income ?? ingameIncome;
    const resultStatus = job.resultStatus || job.status || "-";
    return `
      <tr>
        <td>${formatDate(job.startedAtUtc || job.startedAt || job.started_at_utc)}</td>
        <td>${job.cargo || "-"}</td>
        <td>${route}</td>
        <td>${formatDistance(plannedDistance)}</td>
        <td>${formatCurrency(expectedIncome)}</td>
        <td>${formatCurrency(ingameIncome)}</td>
        <td>${dispatcherStatusMarkup(resultStatus)}</td>
      </tr>
    `;
  });

  setTableRows("cmOrderHistoryBody", historyRows, uiText.noData, 7);
}

function dispatcherFilterPayload() {
  const search = getInputValue("cmDispatcherSearchInput").trim();
  const jobType = getInputValue("cmDispatcherFilterJobType");
  const country = getInputValue("cmDispatcherFilterCountry").trim();
  const sortBy = getInputValue("cmDispatcherSortInput");
  return {
    search: search || null,
    jobType: jobType || null,
    country: country || null,
    sortBy: sortBy || "newest",
  };
}

function titleize(value) {
  return String(value || "-")
    .split("_")
    .filter(Boolean)
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(" ");
}

function normalizeDispatcherJobLinkStatus(value) {
  const normalized = String(value || "").toLowerCase();
  if (!normalized) return "";
  if (["prepared", "written", "pending"].includes(normalized)) return "pending";
  if (normalized === "requires_load") return "requires_load";
  if (["synced", "synced_to_ets2"].includes(normalized)) return "synced";
  if (normalized === "completed") return "completed";
  if (normalized === "error") return "error";
  if (normalized === "pending_route") return "pending_route";
  return normalized;
}

function dispatcherJobLinkLabel(value) {
  switch (normalizeDispatcherJobLinkStatus(value)) {
    case "pending":
      return uiText.dispatcherBadgePending;
    case "requires_load":
      return uiText.dispatcherBadgeRequiresLoad;
    case "synced":
      return uiText.dispatcherBadgeSynced;
    case "completed":
      return uiText.dispatcherBadgeCompleted;
    case "error":
      return uiText.dispatcherBadgeError;
    case "pending_route":
      return uiText.dispatcherBadgePendingRoute;
    default:
      return value ? titleize(value) : uiText.dispatcherUnsynced;
  }
}

function dispatcherJobBadges(job) {
  const badges = [];
  const sourceType = String(job.sourceType || "").toLowerCase();
  if (sourceType === "generated") {
    badges.push(`<span class="dispatcher-badge is-generated">${uiText.dispatcherBadgeGenerated}</span>`);
  } else if (sourceType === "offer") {
    badges.push(`<span class="dispatcher-badge is-offer">${uiText.dispatcherBadgeOffer}</span>`);
  } else if (sourceType === "contract") {
    badges.push(`<span class="dispatcher-badge is-contract">${uiText.dispatcherBadgeContract}</span>`);
  } else if (sourceType === "imported") {
    badges.push(`<span class="dispatcher-badge is-imported">${uiText.dispatcherBadgeImported}</span>`);
  }

  if (["assigned_to_save", "prepared", "injected", "planned", "accepted", "in_transit", "delayed", "problematic"].includes(job.status)) {
    badges.push(`<span class="dispatcher-badge is-active">${uiText.dispatcherBadgeActive}</span>`);
  }

  const linkStatus = normalizeDispatcherJobLinkStatus(job.ets2JobLinkStatus);
  if (linkStatus === "pending") {
    badges.push(`<span class="dispatcher-badge is-link-pending">${uiText.dispatcherBadgePending}</span>`);
  } else if (linkStatus === "requires_load") {
    badges.push(`<span class="dispatcher-badge is-requires-load">${uiText.dispatcherBadgeRequiresLoad}</span>`);
  } else if (linkStatus === "synced") {
    badges.push(`<span class="dispatcher-badge is-synced">${uiText.dispatcherBadgeSynced}</span>`);
  } else if (linkStatus === "completed") {
    badges.push(`<span class="dispatcher-badge is-link-completed">${uiText.dispatcherBadgeCompleted}</span>`);
  } else if (linkStatus === "error") {
    badges.push(`<span class="dispatcher-badge is-link-error">${uiText.dispatcherBadgeError}</span>`);
  } else if (linkStatus === "pending_route") {
    badges.push(`<span class="dispatcher-badge is-pending">${uiText.dispatcherBadgePendingRoute}</span>`);
  }

  if (job.saveReference) {
    badges.push(`<span class="dispatcher-badge is-save">${uiText.dispatcherBadgeSaveLinked}</span>`);
  }

  return badges.join("");
}

function dispatcherStatusMarkup(status) {
  const normalized = String(status || "unknown").toLowerCase();
  return `<span class="dispatcher-status-pill is-${normalized}">${titleize(normalized)}</span>`;
}

function renderDispatcherOverview() {
  const overview = state.dispatcher.overview || {};
  const generation = state.dispatcher.generation || {};
  const context = generation.currentContext || {};
  const saveLinked = Boolean(generation.saveLinkActive);

  setCellText("cmDispatcherOpenJobs", formatNumber(overview.openMarketJobs || 0, 0));
  setCellText("cmDispatcherActiveJobs", formatNumber(overview.activeJobs || 0, 0));
  setCellText("cmDispatcherOpenOffers", formatNumber(overview.openOffers || 0, 0));
  setCellText("cmDispatcherContracts", formatNumber(overview.acceptedContracts || 0, 0));
  setCellText("cmDispatcherSaveRef", saveLinked ? formatPathReference(context.saveReference) : uiText.dispatcherNoSaveLink);
  setCellText("cmDispatcherSaveSession", saveLinked ? (context.saveSessionId || uiText.noData) : uiText.noData);
  setCellText("cmDispatcherNextGeneration", saveLinked ? formatRelativeTime(generation.nextGenerationAtUtc) : uiText.noData);
  setCellText(
    "cmDispatcherGenerationMeta",
    saveLinked
      ? `${uiText.dispatcherIntervalLabel}: ${formatNumber(generation.intervalMinutes || 0, 0)} min | ${uiText.dispatcherPoolLabel}: ${formatNumber(generation.maxOpenJobs || 0, 0)} | ${uiText.dispatcherOpenLabel}: ${formatNumber(generation.openGeneratedJobs || 0, 0)}`
      : uiText.noData
  );

  if (generation.intervalMinutes) {
    setInputValue("cmDispatcherGenerationIntervalInput", String(generation.intervalMinutes));
  }
  if (generation.maxOpenJobs) {
    setInputValue("cmDispatcherGenerationMaxOpenInput", String(generation.maxOpenJobs));
  }
}

function renderDispatcherMarket() {
  const rows = (state.dispatcher.marketJobs || []).map((job) => {
    const route = `${job.originCity || "-"} -> ${job.destinationCity || "-"}`;
    const saveHint = formatPathReference(job.saveReference);
    const badges = dispatcherJobBadges(job);
    const selectedClass = job.id === state.dispatcher.selectedJobId ? "is-selected" : "";
    return `
      <tr class="${selectedClass}" data-dispatcher-job-id="${job.id}">
        <td>${job.id || "-"}</td>
        <td>
          <div class="dispatcher-row-primary">${job.companyName || "-"}</div>
          <div class="dispatcher-row-badges">${badges || uiText.noData}</div>
        </td>
        <td>
          <div class="dispatcher-row-primary">${titleize(job.jobType || "-")}</div>
          <div class="dispatcher-row-secondary">${titleize(job.cargoType || "-")}</div>
        </td>
        <td>
          <div class="dispatcher-row-primary">${route}</div>
          <div class="dispatcher-row-secondary">${saveHint}</div>
        </td>
        <td>${formatDistance(job.distanceKm || 0)}</td>
        <td>${formatNumber(job.calculatedRatePerKm || 0, 2)}</td>
        <td>${formatCurrency(job.totalReward || 0)}</td>
        <td>
          <div class="dispatcher-row-primary">${dispatcherStatusMarkup(job.status)}</div>
          <div class="dispatcher-row-secondary">${formatRelativeTime(job.expiresAtUtc)}</div>
        </td>
      </tr>
    `;
  });
  setTableRows("cmDispatcherMarketBody", rows, uiText.noData, 8);
}

function renderDispatcherDetails() {
  const selected = state.dispatcher.selectedJob;
  const acceptButton = document.getElementById("cmDispatcherAcceptJobBtn");
  const injectButton = document.getElementById("cmDispatcherInjectActiveSaveBtn");
  const syncButton = document.getElementById("cmDispatcherMarkSyncedBtn");
  if (!selected) {
    if (acceptButton) acceptButton.disabled = true;
    if (injectButton) injectButton.disabled = true;
    if (syncButton) syncButton.disabled = true;
    setCellHtml("cmDispatcherDetailBadges", "");
    setCellText("cmDispatcherDetailId", "-");
    setCellText("cmDispatcherDetailSource", "-");
    setCellText("cmDispatcherDetailCompany", "-");
    setCellText("cmDispatcherDetailVtcCompany", "-");
    setCellText("cmDispatcherDetailIngameHost", "-");
    setCellText("cmDispatcherDetailRequestedCargo", "-");
    setCellText("cmDispatcherDetailResolvedCargo", "-");
    setCellText("cmDispatcherDetailCargoResolutionMode", "-");
    setCellText("cmDispatcherDetailCargoValidSnapshot", "-");
    setCellText("cmDispatcherDetailSnapshotSession", "-");
    setCellText("cmDispatcherDetailSnapshotCounts", "-");
    setCellText("cmDispatcherDetailSnapshotDbPath", "-");
    setCellText("cmDispatcherDetailType", "-");
    setCellText("cmDispatcherDetailRoute", "-");
    setCellText("cmDispatcherDetailDistance", "-");
    setCellText("cmDispatcherDetailDuration", "-");
    setCellText("cmDispatcherDetailExpiry", "-");
    setCellText("cmDispatcherDetailRate", "-");
    setCellText("cmDispatcherDetailReward", "-");
    setCellText("cmDispatcherDetailStatus", "-");
    setCellText("cmDispatcherDetailTier", "-");
    setCellText("cmDispatcherDetailReputation", "-");
    setCellText("cmDispatcherDetailProfit", "-");
    setCellText("cmDispatcherDetailSave", "-");
    setCellText("cmDispatcherDetailSaveLinked", "-");
    setCellText("cmDispatcherDetailActiveSave", "-");
    setCellText("cmDispatcherDetailActiveSession", "-");
    setCellText("cmDispatcherDetailRouteRef", "-");
    setCellText("cmDispatcherDetailEts2", "-");
    setCellText("cmDispatcherDetailLastErrorCode", "-");
    setCellText("cmDispatcherDetailLastErrorMessage", "-");
    setCellText("cmDispatcherDetailLiveProgress", "-");
    setCellText("cmDispatcherDetailBonus", "-");
    setCellText("cmDispatcherDetailRisk", "-");
    setCellText("cmDispatcherWriteReport", "-");
    return;
  }

  const job = selected.job || selected;
  const jobLink = state.dispatcher.jobLink;
  const availability = dispatcherActionAvailability(job, jobLink);
  const saveLinked = Boolean(job.linkedToActiveSave || job.saveReference);
  const normalizedStatus = String(job.status || "").toLowerCase();
  const normalizedLinkStatus = String(jobLink?.status || job.ets2JobLinkStatus || "").toLowerCase();
  if (acceptButton) acceptButton.disabled = state.dispatcher.actionBusy || !availability.canAccept;
  if (injectButton) {
    injectButton.disabled = state.dispatcher.actionBusy || !availability.canInject;
    const alreadyInjected = ["injected", "completed"].includes(normalizedStatus)
      || ["requires_load", "synced", "completed", "synced_to_ets2"].includes(normalizedLinkStatus);
    injectButton.textContent = alreadyInjected
      ? uiText.dispatcherAlreadyInjected
      : uiText.dispatcherInjectActiveSave;
  }
  if (syncButton) syncButton.disabled = state.dispatcher.actionBusy || !availability.canMarkSynced;
  const route = `${job.originCity || "-"} -> ${job.destinationCity || "-"}`;
  const ets2StatusLabel = dispatcherJobLinkLabel(jobLink?.status || job.ets2JobLinkStatus);
  const ets2Status = job.lastErrorCode
    ? `${ets2StatusLabel} (${job.lastErrorCode})`
    : ets2StatusLabel;
  const context = state.vtcContext || {};
  const liveProgress = state.dispatcher.liveProgress?.[job.id];
  const eventError = state.dispatcher.lastError?.[job.id];
  setCellHtml("cmDispatcherDetailBadges", dispatcherJobBadges(job));
  setCellText("cmDispatcherDetailId", job.id || "-");
  setCellText("cmDispatcherDetailSource", titleize(job.sourceType || "-"));
  setCellText("cmDispatcherDetailCompany", job.companyName || "-");
  setCellText("cmDispatcherDetailVtcCompany", job.companyId || "-");
  const templateResolved = jobLink?.saveOfferTemplate?.resolved || {};
  const ingameSourceCompany = jobLink?.resolvedSourceCompanyToken
    || templateResolved.resolvedSourceCompanyToken
    || jobLink?.srcCompany
    || "-";
  const ingameSourceCity = jobLink?.resolvedSourceCityToken
    || templateResolved.resolvedSourceCityToken
    || jobLink?.srcCity
    || "-";
  setCellText("cmDispatcherDetailIngameHost", `${ingameSourceCompany}.${ingameSourceCity}`);
  const requestedCargo = jobLink?.requestedCargoToken || job.cargoType || "-";
  const resolvedCargo = jobLink?.resolvedCargoToken || jobLink?.cargoId || "-";
  const cargoResolutionMode = jobLink?.cargoResolutionMode || "-";
  const cargoValid = typeof jobLink?.cargoValidForSnapshot === "boolean"
    ? (jobLink.cargoValidForSnapshot ? "Yes" : "No")
    : "-";
  const snapshotDiagnostics = state.dispatcher.snapshotDiagnostics || null;
  const snapshotCounts = snapshotDiagnostics
    ? `depots=${snapshotDiagnostics.depotCount || 0}, cities=${snapshotDiagnostics.visitedCityCount || 0}, cargo=${snapshotDiagnostics.cargoCount || 0}`
    : "-";
  setCellText("cmDispatcherDetailRequestedCargo", requestedCargo);
  setCellText("cmDispatcherDetailResolvedCargo", resolvedCargo);
  setCellText("cmDispatcherDetailCargoResolutionMode", cargoResolutionMode);
  setCellText("cmDispatcherDetailCargoValidSnapshot", cargoValid);
  setCellText("cmDispatcherDetailSnapshotSession", snapshotDiagnostics?.activeSaveSessionId || context.saveSessionId || "-");
  setCellText("cmDispatcherDetailSnapshotCounts", snapshotCounts);
  setCellText("cmDispatcherDetailSnapshotDbPath", snapshotDiagnostics?.snapshotDbPath || "-");
  setCellText("cmDispatcherDetailType", titleize(job.jobType || "-"));
  setCellText("cmDispatcherDetailRoute", route);
  setCellText("cmDispatcherDetailDistance", formatDistance(job.distanceKm || 0));
  setCellText("cmDispatcherDetailDuration", formatMinutes(job.estimatedDurationMinutes || 0));
  setCellText("cmDispatcherDetailExpiry", formatRelativeTime(job.expiresAtUtc));
  setCellText("cmDispatcherDetailRate", formatNumber(job.calculatedRatePerKm || 0, 2));
  setCellText("cmDispatcherDetailReward", formatCurrency(job.totalReward || 0));
  setCellHtml("cmDispatcherDetailStatus", dispatcherStatusMarkup(job.status));
  setCellText("cmDispatcherDetailTier", job.paymentTierSnapshot || "-");
  setCellText("cmDispatcherDetailReputation", formatNumber(job.companyReputation || 0, 0));
  setCellText("cmDispatcherDetailProfit", formatCurrency(job.profitEstimate || 0));
  setCellText("cmDispatcherDetailSave", formatPathReference(job.saveReference));
  setCellText("cmDispatcherDetailSaveLinked", saveLinked ? "Yes" : "No");
  setCellText("cmDispatcherDetailActiveSave", formatPathReference(context.saveReference || ""));
  setCellText("cmDispatcherDetailActiveSession", context.saveSessionId || uiText.noData);
  setCellText("cmDispatcherDetailRouteRef", job.routeReference || "-");
  setCellText("cmDispatcherDetailEts2", ets2Status);
  setCellText("cmDispatcherDetailLastErrorCode", job.lastErrorCode || eventError?.error || "-");
  setCellText("cmDispatcherDetailLastErrorMessage", job.lastErrorMessage || eventError?.error || "-");
  setCellText("cmDispatcherDetailLiveProgress", liveProgress?.stage || "-");
  setCellText("cmDispatcherDetailBonus", job.bonusNote || "-");
  setCellText("cmDispatcherDetailRisk", job.riskNote || "-");
  setCellText("cmDispatcherWriteReport", formatDispatcherWriteReport(state.dispatcher.writeReport?.[job.id]));
}

function renderDispatcherOffers() {
  const rows = (state.dispatcher.offers || []).map((offer) => {
    const rate = offer.proposedRatePerKm != null ? formatNumber(offer.proposedRatePerKm, 2) : "-";
    const actions = [];
    if (offer.status === "countered") {
      actions.push(`<button class="career-mini-btn" data-offer-accept-counter="${offer.id}">${uiText.dispatcherAccept}</button>`);
      actions.push(`<button class="career-mini-btn" data-offer-reject-counter="${offer.id}">${uiText.dispatcherReject}</button>`);
    }
    if (["draft", "sent", "under_review", "countered"].includes(offer.status)) {
      actions.push(`<button class="career-mini-btn" data-offer-cancel="${offer.id}">${uiText.dispatcherCancel}</button>`);
    }

    return `
      <tr>
        <td>${offer.id || "-"}</td>
        <td>${offer.companyName || "-"}</td>
        <td>${offer.requestedJobType || "-"}</td>
        <td>${rate}</td>
        <td>${offer.status || "-"}</td>
        <td>${actions.join(" ") || "-"}</td>
      </tr>
    `;
  });

  setTableRows("cmDispatcherOffersBody", rows, uiText.noData, 6);
}

function renderDispatcherActive() {
  const rows = (state.dispatcher.activeJobs || []).map((job) => {
    const route = `${job.originCity || "-"} -> ${job.destinationCity || "-"}`;
    const routeDistance = job.routeDistanceKm || job.distanceKm || 0;
    const progress = `${formatDistance(job.progressKm || 0)} / ${formatDistance(routeDistance)}`;
    const selectedClass = job.id === state.dispatcher.selectedJobId ? "is-selected" : "";
    return `
      <tr class="${selectedClass}" data-dispatcher-job-id="${job.id}">
        <td>${job.id || "-"}</td>
        <td>
          <div class="dispatcher-row-primary">${job.companyName || "-"}</div>
          <div class="dispatcher-row-badges">${dispatcherJobBadges(job)}</div>
        </td>
        <td>${titleize(job.jobType || "-")}</td>
        <td>
          <div class="dispatcher-row-primary">${route}</div>
          <div class="dispatcher-row-secondary">${job.routeReference || uiText.noData}</div>
        </td>
        <td>${dispatcherStatusMarkup(job.status)}</td>
        <td>${progress}</td>
      </tr>
    `;
  });
  setTableRows("cmDispatcherActiveBody", rows, uiText.noData, 6);
}

function renderDispatcherContacts() {
  const rows = (state.dispatcher.contacts || []).map((contact) => {
    return `
      <tr>
        <td>${contact.companyName || "-"}</td>
        <td>${contact.paymentTier || "-"}</td>
        <td>${formatNumber(contact.reputation || 0, 0)}</td>
        <td>${formatPercent(contact.successRate || 0)}</td>
        <td>${formatNumber(contact.completedJobs || 0, 0)}</td>
        <td>${formatNumber(contact.failedJobs || 0, 0)}</td>
      </tr>
    `;
  });
  setTableRows("cmDispatcherContactsBody", rows, uiText.noData, 6);
}

function renderDispatcherHistory() {
  const history = state.dispatcher.history || { summary: {}, items: [] };
  const summary = history.summary || {};
  setCellText("cmDispatcherHistCompleted", formatNumber(summary.totalCompleted || 0, 0));
  setCellText("cmDispatcherHistFailed", formatNumber(summary.totalFailed || 0, 0));
  setCellText("cmDispatcherHistRevenue", formatCurrency(summary.revenue || 0));
  setCellText("cmDispatcherHistRate", formatNumber(summary.avgRatePerKm || 0, 2));

  const rows = (history.items || []).map((job) => {
    const route = `${job.originCity || "-"} -> ${job.destinationCity || "-"}`;
    const selectedClass = job.id === state.dispatcher.selectedJobId ? "is-selected" : "";
    return `
      <tr class="${selectedClass}" data-dispatcher-job-id="${job.id}">
        <td>${job.id || "-"}</td>
        <td>
          <div class="dispatcher-row-primary">${job.companyName || "-"}</div>
          <div class="dispatcher-row-badges">${dispatcherJobBadges(job)}</div>
        </td>
        <td>${titleize(job.jobType || "-")}</td>
        <td>
          <div class="dispatcher-row-primary">${route}</div>
          <div class="dispatcher-row-secondary">${formatPathReference(job.saveReference)}</div>
        </td>
        <td>${formatCurrency(job.totalReward || 0)}</td>
        <td>${dispatcherStatusMarkup(job.status)}</td>
      </tr>
    `;
  });
  setTableRows("cmDispatcherHistoryBody", rows, uiText.noData, 6);
}

function renderDispatcher() {
  renderDispatcherOverview();
  renderDispatcherMarket();
  renderDispatcherDetails();
  renderDispatcherOffers();
  renderDispatcherActive();
  renderDispatcherContacts();
  renderDispatcherHistory();
}

function switchDispatcherTab(tab) {
  state.dispatcher.tab = tab;
  document.querySelectorAll(".dispatcher-tab-btn").forEach((button) => {
    button.classList.toggle("is-active", button.dataset.dispatcherTab === tab);
  });
  document.querySelectorAll(".dispatcher-tab-panel").forEach((panel) => {
    panel.classList.toggle("is-active", panel.dataset.dispatcherPanel === tab);
  });
}

async function refreshDispatcherData() {
  const filter = dispatcherFilterPayload();
  state.dispatcher.generation = await safeInvoke(
    "dispatcher_restore_jobs_for_last_quicksave",
    {},
    { fallback: null, silent: true }
  );
  if (!state.dispatcher.generation) {
    state.dispatcher.generation = await safeInvoke(
      "dispatcher_get_generation_status",
      {},
      { fallback: null, silent: true }
    );
  }
  state.dispatcher.overview = await safeInvoke("dispatcher_get_dispatcher_overview", {}, { fallback: null, silent: true });
  state.dispatcher.marketJobs = await safeInvoke(
    "dispatcher_get_open_jobs",
    { filter },
    { fallback: [], silent: true }
  );
  state.dispatcher.activeJobs = await safeInvoke("dispatcher_get_active_jobs", {}, { fallback: [], silent: true });
  state.dispatcher.history = await safeInvoke("dispatcher_get_job_history", {}, { fallback: { summary: {}, items: [] }, silent: true });
  state.dispatcher.contacts = await safeInvoke("dispatcher_get_company_contacts", {}, { fallback: [], silent: true });
  state.dispatcher.offers = await safeInvoke("dispatcher_get_offers", {}, { fallback: [], silent: true });

  if (state.dispatcher.selectedJobId) {
    state.dispatcher.selectedJob = await safeInvoke(
      "dispatcher_get_job_by_id",
      { jobId: state.dispatcher.selectedJobId },
      { fallback: null, silent: true }
    );
    state.dispatcher.jobLink = await safeInvoke(
      "ets_get_job_link_status",
      { vtcJobId: state.dispatcher.selectedJobId },
      { fallback: null, silent: true }
    );
    state.dispatcher.snapshotDiagnostics = await safeInvoke(
      "ets_snapshot_get_active_diagnostics",
      {},
      { fallback: null, silent: true }
    );
    if (!state.dispatcher.selectedJob) {
      state.dispatcher.selectedJobId = null;
      state.dispatcher.jobLink = null;
      state.dispatcher.snapshotDiagnostics = null;
    }
  } else if (state.dispatcher.marketJobs.length > 0) {
    state.dispatcher.selectedJobId = state.dispatcher.marketJobs[0].id;
    state.dispatcher.selectedJob = await safeInvoke(
      "dispatcher_get_job_by_id",
      { jobId: state.dispatcher.selectedJobId },
      { fallback: null, silent: true }
    );
    state.dispatcher.jobLink = await safeInvoke(
      "ets_get_job_link_status",
      { vtcJobId: state.dispatcher.selectedJobId },
      { fallback: null, silent: true }
    );
    state.dispatcher.snapshotDiagnostics = await safeInvoke(
      "ets_snapshot_get_active_diagnostics",
      {},
      { fallback: null, silent: true }
    );
  } else {
    state.dispatcher.selectedJob = null;
    state.dispatcher.jobLink = null;
    state.dispatcher.snapshotDiagnostics = null;
  }
}

async function refreshSelectedDispatcherDetails() {
  if (!state.dispatcher.selectedJobId) return;
  state.dispatcher.selectedJob = await safeInvoke(
    "dispatcher_get_job_by_id",
    { jobId: state.dispatcher.selectedJobId },
    { fallback: state.dispatcher.selectedJob, silent: true }
  );
  state.dispatcher.jobLink = await safeInvoke(
    "ets_get_job_link_status",
    { vtcJobId: state.dispatcher.selectedJobId },
    { fallback: state.dispatcher.jobLink, silent: true }
  );
  state.dispatcher.snapshotDiagnostics = await safeInvoke(
    "ets_snapshot_get_active_diagnostics",
    {},
    { fallback: state.dispatcher.snapshotDiagnostics, silent: true }
  );
}

async function handleDispatcherSelectJob(event, options = {}) {
  const switchToMarket = Boolean(options.switchToMarket);
  const row = event.target.closest("[data-dispatcher-job-id]");
  if (!row) return;
  state.dispatcher.selectedJobId = row.getAttribute("data-dispatcher-job-id");
  state.dispatcher.selectedJob = await safeInvoke(
    "dispatcher_get_job_by_id",
    { jobId: state.dispatcher.selectedJobId },
    { fallback: null, silent: true }
  );
  state.dispatcher.jobLink = await safeInvoke(
    "ets_get_job_link_status",
    { vtcJobId: state.dispatcher.selectedJobId },
    { fallback: null, silent: true }
  );
  state.dispatcher.snapshotDiagnostics = await safeInvoke(
    "ets_snapshot_get_active_diagnostics",
    {},
    { fallback: null, silent: true }
  );
  if (switchToMarket) {
    switchDispatcherTab("market");
  }
  renderDispatcherMarket();
  renderDispatcherActive();
  renderDispatcherHistory();
  renderDispatcherDetails();
}

async function handleDispatcherApplyFilters() {
  await refreshDispatcherData();
  renderDispatcher();
}

async function handleDispatcherResetFilters() {
  setInputValue("cmDispatcherSearchInput", "");
  setInputValue("cmDispatcherFilterJobType", "");
  setInputValue("cmDispatcherFilterCountry", "");
  setInputValue("cmDispatcherSortInput", "newest");
  await refreshDispatcherData();
  renderDispatcher();
}

async function handleDispatcherAcceptJob() {
  if (!state.dispatcher.selectedJobId) return;
  state.dispatcher.actionBusy = true;
  renderDispatcherDetails();
  try {
    const details = await invokeStrict("dispatcher_accept_job", { jobId: state.dispatcher.selectedJobId });
    state.dispatcher.selectedJob = details;
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await t("career_mode.dispatcher.toast_job_accepted"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  } finally {
    state.dispatcher.actionBusy = false;
    renderDispatcherDetails();
  }
}

async function handleDispatcherAssignToActiveSave() {
  if (!state.dispatcher.selectedJobId) return;
  state.dispatcher.liveProgress[state.dispatcher.selectedJobId] = { stage: "assigning", updatedAt: Date.now() };
  state.dispatcher.actionBusy = true;
  renderDispatcherDetails();
  try {
    state.dispatcher.selectedJob = await invokeStrict("dispatcher_assign_job_to_active_save", {
      jobId: state.dispatcher.selectedJobId,
    });
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await t("career_mode.dispatcher.toast_job_assigned"), "success");
  } catch (error) {
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await resolveErrorMessage(error), "error");
  } finally {
    state.dispatcher.actionBusy = false;
    renderDispatcherDetails();
  }
}

async function handleDispatcherInjectToActiveSave() {
  if (!state.dispatcher.selectedJobId) return;
  const jobId = String(state.dispatcher.selectedJobId);
  const context = state.vtcContext || {};
  const autosaveFallbackEnabled = Boolean(getInputChecked("cmDispatcherUseAutosaveFallback"));
  if (!context.quicksaveReference && !autosaveFallbackEnabled) {
    showToast(await t("career_mode.dispatcher.require_quicksave"), "warning");
    return;
  }
  state.dispatcher.liveProgress[jobId] = { stage: "assigning", updatedAt: Date.now() };
  state.dispatcher.actionBusy = true;
  renderDispatcherDetails();
  try {
    console.log("[dispatcher][inject] start", { jobId, autoWrite: true });
    const result = await invokeStrict("dispatcher_assign_and_prepare_and_write", {
      jobId,
      autoWrite: true,
    });
    console.log("[dispatcher][inject] success", result);

    const write = result.writeOutput || null;
    const baseReport = {
      jobId,
      saveReference: result.saveReference || "-",
      quicksaveReference: result.quicksaveReference || "-",
      saveSessionId: result.saveSessionId || "-",
      assignResult: result.assignResult || "-",
      prepareResult: result.prepareResult || "-",
      writeResult: result.writeResult || "-",
      finalDispatcherStatus: result.dispatcherStatus || "-",
      finalLinkStatus: result.ets2JobLinkStatus || "-",
      savePath: result.saveReference || "-",
      backupPath: "-",
      companyPointer: "-",
      offerPointer: "-",
      jobOfferDataPointer: "-",
      origin: "-",
      destination: "-",
      targetCompany: "-",
      cargoToken: "-",
      shortestDistanceKm: "-",
      expirationTime: "-",
      reward: 0,
      writeMode: result.writeResult || "-",
      beforeSha256: result.shaBefore || "-",
      afterSha256: result.shaAfter || "-",
    };

    state.dispatcher.writeReport[jobId] = write
      ? {
          ...baseReport,
          jobId: write.jobId || jobId,
          savePath: write.savePath || baseReport.savePath,
          backupPath: write.backupPath || "-",
          companyPointer: write.companyPointer || "-",
          offerPointer: write.offerPointer || "-",
          jobOfferDataPointer: write.jobOfferDataPointer || "-",
          origin: write.origin || "-",
          destination: write.destination || "-",
          targetCompany: write.targetCompany || "-",
          cargoToken: write.cargoToken || "-",
          shortestDistanceKm: write.shortestDistanceKm ?? "-",
          expirationTime: write.expirationTime ?? "-",
          reward: write.reward ?? 0,
          writeMode: write.writeMode || baseReport.writeMode,
          beforeSha256: write.beforeSha256 || baseReport.beforeSha256,
          afterSha256: write.afterSha256 || baseReport.afterSha256,
          jobInfoUpdated: write.jobInfoUpdated,
          finalLinkStatus: write.finalLinkStatus || baseReport.finalLinkStatus,
        }
      : baseReport;

    state.dispatcher.liveProgress[jobId] = {
      stage: `done (${result.assignResult} -> ${result.prepareResult} -> ${result.writeResult})`,
      updatedAt: Date.now(),
    };
    await refreshDispatcherData();
    renderDispatcher();
    showToast(
      `Injected: ${jobId} | dispatcher=${result.dispatcherStatus} | link=${result.ets2JobLinkStatus || "-"}`,
      "success"
    );
  } catch (error) {
    console.error("[dispatcher][inject] failed", { jobId, error });
    state.dispatcher.liveProgress[jobId] = { stage: "failed", updatedAt: Date.now() };
    state.dispatcher.lastError[jobId] = {
      stage: "inject",
      error: String(error?.message || error || "unknown"),
      updatedAt: Date.now(),
    };
    const raw = String(error?.message || error || "unknown");
    state.dispatcher.writeReport[jobId] = {
      jobId,
      error: raw,
      diagnostics: parseDispatcherDiagnostics(raw),
    };
    await refreshDispatcherData();
    renderDispatcher();
    const resolved = await resolveErrorMessage(error);
    showToast(`${resolved} (${raw})`, "error");
  } finally {
    state.dispatcher.actionBusy = false;
    renderDispatcherDetails();
  }
}

function parseDispatcherDiagnostics(rawError) {
  const out = {};
  const raw = String(rawError || "");
  const segments = raw.split("|").map((part) => part.trim()).filter(Boolean);
  for (const segment of segments) {
    const eq = segment.indexOf("=");
    if (eq <= 0) continue;
    const key = segment.slice(0, eq).trim();
    const value = segment.slice(eq + 1).trim();
    if (!key || !value) continue;
    out[key] = value;
  }
  return out;
}

function dispatcherGenerationConfigPayload() {
  const intervalMinutes = Number(getInputValue("cmDispatcherGenerationIntervalInput"));
  const maxOpenJobs = Number(getInputValue("cmDispatcherGenerationMaxOpenInput"));
  return {
    intervalMinutes: Number.isFinite(intervalMinutes) && intervalMinutes > 0 ? intervalMinutes : null,
    maxOpenJobs: Number.isFinite(maxOpenJobs) && maxOpenJobs > 0 ? maxOpenJobs : null,
  };
}

async function handleDispatcherPersistGenerationConfig() {
  try {
    await invokeStrict("dispatcher_generate_universal_jobs", {
      force: false,
      config: dispatcherGenerationConfigPayload(),
    });
    await refreshDispatcherData();
    renderDispatcher();
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleDispatcherGenerateNow() {
  try {
    state.dispatcher.generation = await invokeStrict("dispatcher_generate_jobs", {});
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await t("career_mode.dispatcher.toast_jobs_generated"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleDispatcherCleanup() {
  try {
    state.dispatcher.generation = await invokeStrict("dispatcher_cleanup_expired_jobs", {});
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await t("career_mode.dispatcher.toast_cleanup_complete"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleDispatcherMarkSynced() {
  if (!state.dispatcher.selectedJobId) return;
  state.dispatcher.actionBusy = true;
  renderDispatcherDetails();
  try {
    const selected = state.dispatcher.selectedJob?.job || state.dispatcher.selectedJob || {};
    state.dispatcher.selectedJob = await invokeStrict("dispatcher_mark_job_synced_to_ets2", {
      jobId: state.dispatcher.selectedJobId,
      routeReference: selected.routeReference || null,
    });
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await t("career_mode.dispatcher.toast_job_synced"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  } finally {
    state.dispatcher.actionBusy = false;
    renderDispatcherDetails();
  }
}

async function handleDispatcherPrepareEtsLink() {
  if (!state.dispatcher.selectedJobId) return;
  state.dispatcher.liveProgress[state.dispatcher.selectedJobId] = { stage: "preparing", updatedAt: Date.now() };
  state.dispatcher.actionBusy = true;
  renderDispatcherDetails();
  try {
    state.dispatcher.jobLink = await invokeStrict("ets_prepare_job_link", {
      vtcJobId: state.dispatcher.selectedJobId,
      profileId: "",
    });
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await t("career_mode.dispatcher.toast_link_prepared"), "success");
  } catch (error) {
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await resolveErrorMessage(error), "error");
  } finally {
    state.dispatcher.actionBusy = false;
    renderDispatcherDetails();
  }
}

async function handleDispatcherWriteQuicksave() {
  if (!state.dispatcher.selectedJobId) return;
  state.dispatcher.liveProgress[state.dispatcher.selectedJobId] = { stage: "writing", updatedAt: Date.now() };
  state.dispatcher.actionBusy = true;
  renderDispatcherDetails();
  try {
    if (!state.dispatcher.jobLink?.linkId) {
      state.dispatcher.jobLink = await invokeStrict("ets_get_job_link_status", {
        vtcJobId: state.dispatcher.selectedJobId,
      });
    }
    const writeResult = await invokeStrict("ets_write_job_to_quicksave", {
      linkId: state.dispatcher.jobLink.linkId,
    });
    state.dispatcher.jobLink = writeResult.link || writeResult;
    const selected = currentDispatcherJob() || {};
    const patch = writeResult.link?.patch || {};
    state.dispatcher.writeReport[state.dispatcher.selectedJobId] = {
      jobId: state.dispatcher.selectedJobId,
      savePath: writeResult.savePath || selected.saveReference || "",
      backupPath: writeResult.backupPath || "",
      companyPointer: `company.volatile.${writeResult.link?.srcCompany || selected.companyId || "-"}.${String(selected.originCity || "").toLowerCase()}`,
      offerPointer: writeResult.link?.offerPointer || "-",
      jobOfferDataPointer: writeResult.link?.jobOfferDataPointer || "-",
      origin: `${selected.originCity || "-"} (${selected.originCountry || "-"})`,
      destination: `${selected.destinationCity || "-"} (${selected.destinationCountry || "-"})`,
      targetCompany: patch.target || writeResult.link?.dstCompany || "-",
      cargoToken: patch.cargo || writeResult.link?.cargoId || "-",
      shortestDistanceKm: patch.shortestDistanceKm ?? Math.round(Number(selected.distanceKm || 0)),
      expirationTime: patch.expirationTime ?? "-",
      reward: selected.totalReward || writeResult.link?.plannedReward || 0,
      writeMode: writeResult.writeMode || "overwrite_existing_offer",
      beforeSha256: writeResult.beforeSha256 || "-",
      afterSha256: writeResult.afterSha256 || "-",
      finalLinkStatus: writeResult.link?.status || "-",
    };
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await t("career_mode.dispatcher.toast_quicksave_load_required"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  } finally {
    state.dispatcher.actionBusy = false;
    renderDispatcherDetails();
  }
}

async function handleDispatcherSendOffer() {
  try {
    const companyId = getInputValue("cmDispatcherOfferCompanyInput").trim();
    if (!companyId) return;
    const proposedRate = Number(getInputValue("cmDispatcherOfferRateInput"));
    const payload = {
      companyId,
      offerType: "quote_request",
      requestedJobType: getInputValue("cmDispatcherOfferJobTypeInput"),
      requestedRegion: getInputValue("cmDispatcherOfferRegionInput").trim() || null,
      proposedRatePerKm: Number.isFinite(proposedRate) && proposedRate > 0 ? proposedRate : null,
      note: getInputValue("cmDispatcherOfferNoteInput").trim() || null,
      equipmentType: getInputValue("cmDispatcherOfferEquipmentInput"),
      contractScope: getInputValue("cmDispatcherOfferScopeInput"),
    };
    await invokeStrict("dispatcher_create_offer", { input: payload });
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await t("career_mode.dispatcher.toast_offer_sent"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleDispatcherOfferActions(event) {
  const cancelButton = event.target.closest("[data-offer-cancel]");
  const acceptCounterButton = event.target.closest("[data-offer-accept-counter]");
  const rejectCounterButton = event.target.closest("[data-offer-reject-counter]");

  try {
    if (cancelButton) {
      await invokeStrict("dispatcher_cancel_offer", { offerId: cancelButton.getAttribute("data-offer-cancel") });
      await refreshDispatcherData();
      renderDispatcher();
      return;
    }
    if (acceptCounterButton) {
      await invokeStrict("dispatcher_respond_to_counter", {
        input: {
          offerId: acceptCounterButton.getAttribute("data-offer-accept-counter"),
          acceptCounter: true,
        },
      });
      await refreshDispatcherData();
      renderDispatcher();
      return;
    }
    if (rejectCounterButton) {
      await invokeStrict("dispatcher_respond_to_counter", {
        input: {
          offerId: rejectCounterButton.getAttribute("data-offer-reject-counter"),
          acceptCounter: false,
        },
      });
      await refreshDispatcherData();
      renderDispatcher();
    }
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

function renderFinances() {
  const overview = state.overview || {};
  const dashboard = overview.dashboard || {};
  const stats = state.jobStats || overview.jobStats || {};

  setCellText("cmFinanceIncome", formatCurrency(dashboard.liveIncome || stats.totalIncome || 0));
  setCellText("cmFinanceFuel", formatCurrency(dashboard.fuelCost || 0));
  setCellText("cmFinanceRepair", formatCurrency(dashboard.repairCost || 0));
  setCellText("cmFinanceToll", formatCurrency(dashboard.tollCost || 0));

  const rows = [
    ["Cash Balance", formatCurrency(overview.bank?.cashBalance || 0)],
    ["Insurance / Day", formatCurrency(overview.economy?.insuranceDailyCost || 0)],
    ["Total Jobs", formatNumber(stats.totalJobs || 0, 0)],
    ["Success Rate", `${Math.round(Number(stats.successRate || 0) * 100)}%`],
  ].map(([label, value]) => `<tr><td>${label}</td><td>${value}</td></tr>`);

  setTableRows("cmFinanceSummaryBody", rows, uiText.noData, 2);
}

function renderFleet() {
  const fleet = Array.isArray(state.overview?.fleet) ? state.overview.fleet : [];

  const rows = fleet.map((asset) => {
    const assetName = `${asset.brand || "-"} ${asset.model || ""}`.trim();
    const condition = `${formatNumber(asset.conditionPercent || 0, 1)}%`;
    const distance = formatDistance(asset.serviceDueKm || 0);
    return `
      <tr>
        <td>${asset.kind || "-"}</td>
        <td>${assetName}</td>
        <td>${condition}</td>
        <td>${distance}</td>
      </tr>
    `;
  });

  setTableRows("cmFleetBody", rows, uiText.noData, 4);
}

function renderCompany() {
  const company = state.companyOverview;

  setInputValue("cmCompanyNameInput", company?.name);
  setInputValue("cmCompanyLocationInput", company?.location);
  setInputValue("cmCompanyLanguageInput", company?.language || "en");
  setInputValue("cmCompanyGameInput", company?.game || "ETS2");
  setInputValue("cmCompanyLogoInput", company?.logoPath);
  setInputValue("cmCompanyHeaderInput", company?.headerPath);
  setInputValue("cmCompanySloganInput", company?.slogan);
  setInputValue("cmCompanyAccentInput", company?.accentColor);
  setInputValue("cmCompanyDescriptionInput", company?.description);
  setInputChecked("cmCompanyPublicVisibilityInput", company?.publicVisibility ?? true);

  const button = document.getElementById("cmCompanyProfileSaveBtn");
  if (button) {
    button.textContent = company ? "Save Company Profile" : "Create Company";
  }

  const settings = state.companySettings;
  renderRoleOptions("cmCompanyDefaultRoleInput", settings?.defaultMemberRole || "driver");
  setInputChecked("cmCompanyAllowJoinRequestsInput", settings?.allowPublicJoinRequests ?? false);
  setInputChecked("cmCompanyDispatcherManageJobsInput", settings?.dispatcherCanManageJobs ?? true);
  setInputChecked("cmCompanyShowTraineeInput", settings?.traineeVisibleInRoster ?? true);
  setInputChecked("cmCompanyAllowCustomProfilesInput", settings?.allowMemberCustomProfiles ?? true);
}

function renderStatistics() {
  const statistics = state.overview?.statistics || {};
  const totalTrips = statistics.totalTrips ?? statistics.completedTrips ?? 0;

  setCellText("cmStatTrips", formatNumber(totalTrips || 0, 0));
  setCellText("cmStatKm", formatNumber(statistics.totalKilometers || 0, 0));
  setCellText("cmStatRevenue", formatCurrency(statistics.totalIncome || 0));
  setCellText("cmStatCompanyValue", formatCurrency(statistics.companyValue || 0));
  setCellText("cmStatSpeed", `${formatNumber(statistics.averageSpeed || 0, 1)} km/h`);
  setCellText("cmStatSpeeding", formatNumber(statistics.speedingEvents || 0, 0));

  const bars = [
    Number(totalTrips || 0),
    Number(statistics.totalKilometers || 0) / 100,
    Number(statistics.totalIncome || 0) / 1000,
    Number(statistics.companyValue || 0) / 1000,
    Number(statistics.averageSpeed || 0),
    Number(statistics.speedingEvents || 0) * 10,
  ];
  const max = Math.max(1, ...bars);

  const chartHost = document.getElementById("cmChartBars");
  if (!chartHost) return;
  chartHost.innerHTML = bars
    .map((value) => {
      const percent = Math.max(8, Math.round((value / max) * 100));
      return `<div class="career-chart-bar" style="height:${percent}%"></div>`;
    })
    .join("");
}

function renderStatus() {
  const status = state.status || {};

  const gameRunning = Boolean(status.ets2_running || status.ats_running);
  const pluginInstalled = Boolean(status.plugin_installed);
  const bridgeConnected = Boolean(status.bridge_connected);
  const activeGame = String(status.active_game || "ETS2").toUpperCase();

  setCellText("careerStatusGameRunning", gameRunning ? uiText.online : uiText.offline);
  setCellText("careerStatusPluginInstalled", pluginInstalled ? uiText.online : uiText.offline);
  setCellText("careerStatusBridgeConnected", bridgeConnected ? uiText.online : uiText.offline);
  setCellText("careerStatusActiveGame", activeGame);
  setCellText("careerDataSource", state.source === "live" ? uiText.sourceLive : uiText.sourceMock);
}

function renderVtcContext() {
  const context = state.vtcContext || {};
  const userLabel = context.username
    ? `${context.username} #${formatNumber(context.userId || 0, 0)}`
    : uiText.noData;

  setCellText("careerVtcContextUser", userLabel);
  setCellText(
    "careerVtcContextProfile",
    formatVtcPathReference(context.profileReference, uiText.vtcContextNoProfile)
  );
  setCellText(
    "careerVtcContextSave",
    formatVtcPathReference(context.saveReference, uiText.vtcContextNoSave)
  );
  setCellText("careerVtcContextSession", context.saveSessionId || uiText.noData);
  setCellText("careerVtcContextSessionStatus", sessionStatusLabel(context.saveSessionStatus));
}

function setUserMenuOpen(open) {
  const dropdown = document.getElementById("careerUserMenuDropdown");
  const button = document.getElementById("careerUserMenuBtn");
  if (!dropdown || !button) return;
  const visible = Boolean(open);
  dropdown.hidden = !visible;
  button.setAttribute("aria-expanded", visible ? "true" : "false");
}

function toggleUserMenu() {
  const dropdown = document.getElementById("careerUserMenuDropdown");
  setUserMenuOpen(Boolean(dropdown?.hidden));
}

function renderUserMenu() {
  const user = state.authUser || null;
  const label = user ? uiText.userMenuAccount : uiText.userMenuLogin;
  const displayName = user
    ? (user.username || user.email || uiText.userMenuAccount)
    : uiText.userMenuNotLoggedIn;

  setCellText("careerUserMenuLabel", label);
  setCellText("careerUserMenuName", displayName);
  setCellText("careerUserMenuIdentity", user ? (user.email || user.username || "-") : uiText.userMenuNotLoggedIn);
  setCellText("careerUserMenuState", user ? (user.role || uiText.userMenuAccount) : uiText.userMenuLogin);

  const loginButton = document.getElementById("careerUserMenuLogin");
  const profileButton = document.getElementById("careerUserMenuProfile");
  const settingsButton = document.getElementById("careerUserMenuSettings");
  const logoutButton = document.getElementById("careerUserMenuLogout");

  if (loginButton) loginButton.hidden = Boolean(user);
  if (logoutButton) logoutButton.hidden = !Boolean(user);
  if (profileButton) profileButton.disabled = !Boolean(user);
  if (settingsButton) settingsButton.disabled = false;
}

function renderUserSettings() {
  const profile = state.userProfile;
  const settings = state.userSettings;

  setInputValue("cmUserUsernameInput", profile?.username);
  setInputValue("cmUserLanguageInput", settings?.language || profile?.language || "en");
  setInputValue("cmUserPreferredGameInput", settings?.preferredGame || "ETS2");
  setInputValue("cmUserAvatarInput", settings?.avatarPath || profile?.avatarPath || "");
  setInputValue("cmUserVisibilityInput", settings?.profileVisibility || "private");
  setInputValue("cmUserBioInput", settings?.bio || profile?.bio || "");
  setInputValue("cmUserThemeInput", settings?.themePreference || "");
  setInputChecked("cmUserNotificationsInput", settings?.notificationsEnabled ?? true);

  const cooldownHint = document.getElementById("cmUsernameCooldownHint");
  if (cooldownHint) {
    cooldownHint.textContent = profile?.usernameNextChangeAt
      ? `Next change: ${formatDate(profile.usernameNextChangeAt)}`
      : "Username can be changed now";
  }
}

function renderCareerSettings() {
  const settings = state.careerSettings;
  setInputChecked("cmCareerTelemetryEnabledInput", settings?.telemetryEnabled ?? true);
  setInputChecked("cmCareerLocalStatsTrackingInput", settings?.localStatsTrackingEnabled ?? true);
  setInputChecked("cmCareerAutoJobLoggingInput", settings?.autoJobLoggingEnabled ?? true);
  setInputChecked("cmCareerAutoFinanceTrackingInput", settings?.autoFinanceTrackingEnabled ?? true);
  setInputChecked("cmCareerMetricUnitsInput", settings?.useMetricUnits ?? true);
  setInputChecked("cmCareer24hTimeInput", settings?.use24hTime ?? true);
  setInputChecked("cmCareerAutosaveInput", settings?.autosaveCareerData ?? true);

  renderRoleOptions("cmAssignRoleSelect", "driver");
}

function switchPanel(panel) {
  document.querySelectorAll(".career-nav-btn").forEach((button) => {
    button.classList.toggle("is-active", button.dataset.panel === panel);
  });

  document.querySelectorAll(".career-panel").forEach((section) => {
    section.classList.toggle("is-active", section.id === `panel-${panel}`);
  });
}

function applyNavHandlers() {
  document.querySelectorAll(".career-nav-btn").forEach((button) => {
    button.addEventListener("click", () => {
      switchPanel(button.dataset.panel);
    });
  });
}

function applyDispatcherHandlers() {
  document.querySelectorAll(".dispatcher-tab-btn").forEach((button) => {
    button.addEventListener("click", () => {
      switchDispatcherTab(button.dataset.dispatcherTab);
    });
  });

  document.getElementById("cmDispatcherMarketBody")?.addEventListener("click", (event) => {
    void handleDispatcherSelectJob(event);
  });
  document.getElementById("cmDispatcherActiveBody")?.addEventListener("click", (event) => {
    void handleDispatcherSelectJob(event, { switchToMarket: true });
  });
  document.getElementById("cmDispatcherHistoryBody")?.addEventListener("click", (event) => {
    void handleDispatcherSelectJob(event, { switchToMarket: true });
  });
  document.getElementById("cmDispatcherApplyFiltersBtn")?.addEventListener("click", () => {
    void handleDispatcherApplyFilters();
  });
  document.getElementById("cmDispatcherResetFiltersBtn")?.addEventListener("click", () => {
    void handleDispatcherResetFilters();
  });
  document.getElementById("cmDispatcherAcceptJobBtn")?.addEventListener("click", () => {
    void handleDispatcherAcceptJob();
  });
  document.getElementById("cmDispatcherInjectActiveSaveBtn")?.addEventListener("click", () => {
    void handleDispatcherInjectToActiveSave();
  });
  document.getElementById("cmDispatcherMarkSyncedBtn")?.addEventListener("click", () => {
    void handleDispatcherMarkSynced();
  });
  document.getElementById("cmDispatcherSendOfferBtn")?.addEventListener("click", () => {
    void handleDispatcherSendOffer();
  });
  document.getElementById("cmDispatcherGenerateNowBtn")?.addEventListener("click", () => {
    void handleDispatcherGenerateNow();
  });
  document.getElementById("cmDispatcherCleanupBtn")?.addEventListener("click", () => {
    void handleDispatcherCleanup();
  });
  document.getElementById("cmDispatcherGenerationIntervalInput")?.addEventListener("change", () => {
    void handleDispatcherPersistGenerationConfig();
  });
  document.getElementById("cmDispatcherGenerationMaxOpenInput")?.addEventListener("change", () => {
    void handleDispatcherPersistGenerationConfig();
  });
  document.getElementById("cmDispatcherOffersBody")?.addEventListener("click", (event) => {
    void handleDispatcherOfferActions(event);
  });
}

function applySettingsHandlers() {
  const autoRefreshCheckbox = document.getElementById("cmSettingAutoRefresh");
  autoRefreshCheckbox?.addEventListener("change", () => {
    if (autoRefreshCheckbox.checked) {
      startRefreshLoop();
    } else {
      stopRefreshLoop();
    }
  });
}

let refreshTimer = null;
let dispatcherClockTimer = null;

function stopRefreshLoop() {
  if (!refreshTimer) return;
  clearInterval(refreshTimer);
  refreshTimer = null;
}

function startRefreshLoop() {
  stopRefreshLoop();
  refreshTimer = setInterval(() => {
    const enabled = getInputChecked("cmSettingAutoRefresh");
    if (enabled) {
      void refreshAllData();
    }
  }, 12000);
}

function startDispatcherClock() {
  if (dispatcherClockTimer) return;
  dispatcherClockTimer = setInterval(() => {
    renderDispatcherOverview();
    renderDispatcherMarket();
    renderDispatcherDetails();
  }, 1000);
}

function registerDispatcherEventHandlers() {
  if (!hasTauri || window.__dispatcher_listeners_registered) return;
  window.__dispatcher_listeners_registered = true;

  listen("career://dispatcher_changed", async (event) => {
    console.log("[dispatcher][event] career://dispatcher_changed", event.payload);
    state.dispatcher.generation = event.payload || null;
    await refreshDispatcherData();
    renderDispatcher();
  }).catch(console.error);
  listen("vtc://dispatcher/job_link_updated", async () => {
    console.log("[dispatcher][event] vtc://dispatcher/job_link_updated");
    await refreshDispatcherData();
    renderDispatcher();
  }).catch(console.error);
  listen("vtc://dispatcher/job_updated", async (event) => {
    console.log("[dispatcher][event] vtc://dispatcher/job_updated", event.payload);
    const payload = event.payload || {};
    if (payload.jobId && state.dispatcher.selectedJobId && String(payload.jobId) === String(state.dispatcher.selectedJobId)) {
      await refreshSelectedDispatcherDetails();
      renderDispatcherDetails();
      renderDispatcherMarket();
      renderDispatcherActive();
      renderDispatcherHistory();
      return;
    }
    await refreshDispatcherData();
    renderDispatcher();
  }).catch(console.error);
  listen("vtc://dispatcher/jobs_updated", async () => {
    console.log("[dispatcher][event] vtc://dispatcher/jobs_updated");
    await refreshDispatcherData();
    renderDispatcher();
  }).catch(console.error);
  listen("vtc://dispatcher/assign_progress", async (event) => {
    console.log("[dispatcher][event] vtc://dispatcher/assign_progress", event.payload);
    const payload = event.payload || {};
    const jobId = payload.jobId ? String(payload.jobId) : "";
    if (!jobId) return;
    state.dispatcher.liveProgress[jobId] = {
      stage: String(payload.stage || "assigning"),
      updatedAt: Date.now(),
    };
    if (!state.dispatcher.selectedJobId || String(state.dispatcher.selectedJobId) === jobId) {
      await refreshSelectedDispatcherDetails();
      renderDispatcherDetails();
      renderDispatcherMarket();
      showToast(`Assign: ${payload.stage || "running"} (${jobId})`, "info");
    }
  }).catch(console.error);
  listen("vtc://dispatcher/prepare_progress", async (event) => {
    console.log("[dispatcher][event] vtc://dispatcher/prepare_progress", event.payload);
    const payload = event.payload || {};
    const jobId = payload.jobId ? String(payload.jobId) : "";
    if (!jobId) return;
    state.dispatcher.liveProgress[jobId] = {
      stage: String(payload.stage || "preparing"),
      updatedAt: Date.now(),
    };
    if (!state.dispatcher.selectedJobId || String(state.dispatcher.selectedJobId) === jobId) {
      await refreshSelectedDispatcherDetails();
      renderDispatcherDetails();
      renderDispatcherMarket();
      showToast(`Prepare: ${payload.stage || "running"} (${jobId})`, "info");
    }
  }).catch(console.error);
  listen("vtc://dispatcher/write_progress", async (event) => {
    console.log("[dispatcher][event] vtc://dispatcher/write_progress", event.payload);
    const payload = event.payload || {};
    const jobId = payload.jobId ? String(payload.jobId) : "";
    if (!jobId) return;
    state.dispatcher.liveProgress[jobId] = {
      stage: String(payload.stage || "writing"),
      updatedAt: Date.now(),
    };
    if (!state.dispatcher.selectedJobId || String(state.dispatcher.selectedJobId) === jobId) {
      await refreshSelectedDispatcherDetails();
      renderDispatcherDetails();
      renderDispatcherMarket();
      renderDispatcherActive();
      showToast(`Write: ${payload.stage || "running"} (${jobId})`, "info");
    }
  }).catch(console.error);
  listen("vtc://dispatcher/job_error", async (event) => {
    console.error("[dispatcher][event] vtc://dispatcher/job_error", event.payload);
    const payload = event.payload || {};
    const jobId = payload.jobId ? String(payload.jobId) : "";
    if (!jobId) return;
    state.dispatcher.lastError[jobId] = {
      stage: String(payload.stage || "error"),
      error: String(payload.error || "unknown"),
      updatedAt: Date.now(),
    };
    if (!state.dispatcher.selectedJobId || String(state.dispatcher.selectedJobId) === jobId) {
      await refreshSelectedDispatcherDetails();
      renderDispatcherDetails();
    }
    showToast(String(payload.error || "Dispatcher error"), "error");
  }).catch(console.error);
}

function registerCareerEventHandlers() {
  if (!hasTauri || window.__career_listeners_registered) return;
  window.__career_listeners_registered = true;

  listen("career://status", (event) => {
    state.status = event.payload || null;
    renderStatus();
  }).catch(console.error);

  listen("career://overview", (event) => {
    const nextOverview = event.payload || null;
    if (!nextOverview) return;
    state.overview = nextOverview;
    state.source = "live";
    if (Array.isArray(nextOverview.recentJobs)) {
      state.jobLog = nextOverview.recentJobs;
    }
    if (nextOverview.jobStats) {
      state.jobStats = nextOverview.jobStats;
    }
    renderAll();
  }).catch(console.error);

  listen("vtc://system/status", async () => {
    state.vtcContext = await safeInvoke("get_vtc_runtime_context", {}, { fallback: null, silent: true });
    renderStatus();
    renderVtcContext();
  }).catch(console.error);
}

async function loadUiText() {
  uiText.online = await t("career_mode.online");
  uiText.offline = await t("career_mode.offline");
  uiText.noData = await t("career.shared.none");
  uiText.sourceLive = await t("career_mode.source_live");
  uiText.sourceMock = await t("career_mode.source_mock");
  uiText.companyLiveDesc = await t("career_mode.company_live_desc");
  uiText.companyMockDesc = await t("career_mode.company_mock_desc");
  uiText.genericSaved = await t("career_mode.apply_role");
  uiText.errorPrefix = await t("career_mode.error_prefix");
  uiText.dispatcherAccept = await t("career_mode.dispatcher.action_accept");
  uiText.dispatcherReject = await t("career_mode.dispatcher.action_reject");
  uiText.dispatcherCancel = await t("career_mode.dispatcher.action_cancel");
  uiText.dispatcherBadgeGenerated = await t("career_mode.dispatcher.badge_generated");
  uiText.dispatcherBadgeOffer = await t("career_mode.dispatcher.badge_offer");
  uiText.dispatcherBadgeContract = await t("career_mode.dispatcher.badge_contract");
  uiText.dispatcherBadgeImported = await t("career_mode.dispatcher.badge_imported");
  uiText.dispatcherBadgeActive = await t("career_mode.dispatcher.badge_active");
  uiText.dispatcherBadgePending = await t("career_mode.dispatcher.badge_pending");
  uiText.dispatcherBadgeRequiresLoad = await t("career_mode.dispatcher.badge_requires_load");
  uiText.dispatcherBadgeSynced = await t("career_mode.dispatcher.badge_synced");
  uiText.dispatcherBadgeCompleted = await t("career_mode.dispatcher.badge_completed");
  uiText.dispatcherBadgeError = await t("career_mode.dispatcher.badge_error");
  uiText.dispatcherBadgePendingRoute = await t("career_mode.dispatcher.badge_pending_route");
  uiText.dispatcherBadgeSaveLinked = await t("career_mode.dispatcher.badge_save_linked");
  uiText.dispatcherNoSaveLink = await t("career_mode.dispatcher.no_save_link");
  uiText.dispatcherUnsynced = await t("career_mode.dispatcher.unsynced");
  uiText.dispatcherExpired = await t("career_mode.dispatcher.expired");
  uiText.dispatcherInjectActiveSave = await t("career_mode.dispatcher.inject_active_save");
  uiText.dispatcherAlreadyInjected = await t("career_mode.dispatcher.already_injected");
  uiText.dispatcherPrepareLink = await t("career_mode.dispatcher.prepare_ets_link");
  uiText.dispatcherRetryPrepareLink = await t("career_mode.dispatcher.retry_prepare_ets_link");
  uiText.dispatcherWriteQuicksave = await t("career_mode.dispatcher.write_to_quicksave");
  uiText.dispatcherIntervalLabel = await t("career_mode.dispatcher.generation_interval_short");
  uiText.dispatcherPoolLabel = await t("career_mode.dispatcher.generation_pool_short");
  uiText.dispatcherOpenLabel = await t("career_mode.dispatcher.generation_open_short");
  uiText.vtcContextNoProfile = await t("career_mode.vtc_context_no_profile");
  uiText.vtcContextNoSave = await t("career_mode.vtc_context_no_save");
  uiText.vtcContextSessionLinked = await t("career_mode.vtc_context_status_linked");
  uiText.vtcContextSessionInferred = await t("career_mode.vtc_context_status_inferred");
  uiText.vtcContextSessionProfileOnly = await t("career_mode.vtc_context_status_profile_only");
  uiText.vtcContextSessionMissing = await t("career_mode.vtc_context_status_missing");
  uiText.userMenuAccount = await t("career.user_menu.account");
  uiText.userMenuLogin = await t("career.user_menu.login");
  uiText.userMenuNotLoggedIn = await t("career.user_menu.not_logged_in");
}

async function refreshCareerTelemetryData() {
  const status = await safeInvoke("career_get_status", {}, { fallback: null, silent: true });
  const overview = await safeInvoke("career_get_overview", {}, { fallback: null, silent: true });
  const jobLog = await safeInvoke("career_get_job_log", {}, { fallback: [], silent: true });
  const jobStats = await safeInvoke("career_get_job_stats", {}, { fallback: null, silent: true });

  const allowMock = getInputChecked("cmSettingMockFallback");
  const hasLiveOverview = Boolean(overview);

  state.status = status || null;
  state.overview = hasLiveOverview
    ? overview
    : allowMock
      ? buildFallbackOverview()
      : { ...buildFallbackOverview(), recentJobs: [] };
  state.jobLog = hasLiveOverview && Array.isArray(state.overview?.recentJobs) && state.overview.recentJobs.length > 0
    ? state.overview.recentJobs
    : Array.isArray(jobLog) && jobLog.length > 0
      ? jobLog
      : state.overview.recentJobs || [];
  state.jobStats = jobStats || state.overview.jobStats || null;
  state.source = hasLiveOverview ? "live" : "mock";

  if (state.jobStats && state.overview) {
    state.overview.jobStats = state.jobStats;
  }
}

async function refreshManagementData() {
  await safeInvoke("auth_restore_session", {}, { silent: true });
  state.authUser = await safeInvoke("auth_get_current_user", {}, { fallback: null, silent: true });
  state.vtcContext = await safeInvoke("get_vtc_runtime_context", {}, { fallback: null, silent: true });
  state.roles = await safeInvoke("get_available_roles", {}, { fallback: [], silent: true });
  state.userProfile = await safeInvoke("get_current_user_profile", {}, { fallback: null, silent: true });
  state.userSettings = await safeInvoke("get_user_settings", {}, { fallback: null, silent: true });
  state.companyOverview = await safeInvoke("get_company_overview", {}, { fallback: null, silent: true });
  state.companyMembers = await safeInvoke("get_company_members", {}, { fallback: [], silent: true });
  state.companySettings = await safeInvoke("get_company_settings", {}, { fallback: null, silent: true });
  state.careerSettings = await safeInvoke("get_career_settings", {}, { fallback: null, silent: true });
}

function renderAll() {
  renderStatus();
  renderVtcContext();
  renderUserMenu();
  renderDashboard();
  renderMembers();
  renderOrders();
  renderDispatcher();
  renderFinances();
  renderFleet();
  renderCompany();
  renderStatistics();
  renderUserSettings();
  renderCareerSettings();
}

async function refreshAllData() {
  await refreshCareerTelemetryData();
  await refreshManagementData();
  await refreshDispatcherData();
  renderAll();
}

async function handleUpdateUsername() {
  try {
    const username = getInputValue("cmUserUsernameInput").trim();
    if (!username) return;
    await invokeStrict("update_username", { username });
    await refreshAllData();
    showToast(await t("career_mode.user_saved"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleSaveUserSettings() {
  try {
    const language = getInputValue("cmUserLanguageInput");
    const preferredGame = getInputValue("cmUserPreferredGameInput");
    const avatarPath = getInputValue("cmUserAvatarInput").trim() || null;
    const profileVisibility = getInputValue("cmUserVisibilityInput");
    const bio = getInputValue("cmUserBioInput").trim() || null;
    const themePreference = getInputValue("cmUserThemeInput").trim() || null;
    const notificationsEnabled = getInputChecked("cmUserNotificationsInput");

    await invokeStrict("update_user_language", { language });
    await invokeStrict("update_user_profile_meta", {
      input: {
        avatarPath,
        bio,
        profileVisibility,
      },
    });
    await invokeStrict("update_user_settings", {
      input: {
        language,
        preferredGame,
        profileVisibility,
        themePreference,
        notificationsEnabled,
      },
    });

    await refreshAllData();
    showToast(await t("career_mode.user_saved"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleSaveCompanyProfile() {
  try {
    const payload = {
      name: getInputValue("cmCompanyNameInput").trim(),
      location: getInputValue("cmCompanyLocationInput").trim(),
      language: getInputValue("cmCompanyLanguageInput"),
      game: getInputValue("cmCompanyGameInput"),
      description: getInputValue("cmCompanyDescriptionInput").trim() || null,
      logoPath: getInputValue("cmCompanyLogoInput").trim() || null,
      headerPath: getInputValue("cmCompanyHeaderInput").trim() || null,
      slogan: getInputValue("cmCompanySloganInput").trim() || null,
      accentColor: getInputValue("cmCompanyAccentInput").trim() || null,
      publicVisibility: getInputChecked("cmCompanyPublicVisibilityInput"),
    };

    if (state.companyOverview) {
      await invokeStrict("update_company_profile", { input: payload });
    } else {
      await invokeStrict("create_company", { input: payload });
    }

    await refreshAllData();
    showToast(await t("career_mode.company_saved"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleSaveCompanySettings() {
  try {
    await invokeStrict("update_company_settings", {
      input: {
        defaultMemberRole: getInputValue("cmCompanyDefaultRoleInput"),
        allowPublicJoinRequests: getInputChecked("cmCompanyAllowJoinRequestsInput"),
        dispatcherCanManageJobs: getInputChecked("cmCompanyDispatcherManageJobsInput"),
        traineeVisibleInRoster: getInputChecked("cmCompanyShowTraineeInput"),
        allowMemberCustomProfiles: getInputChecked("cmCompanyAllowCustomProfilesInput"),
        showCompanyPublicly: getInputChecked("cmCompanyPublicVisibilityInput"),
        companyLanguage: getInputValue("cmCompanyLanguageInput"),
        companyGame: getInputValue("cmCompanyGameInput"),
      },
    });

    await refreshAllData();
    showToast(await t("career_mode.company_settings_saved"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleSaveCareerSettings() {
  try {
    await invokeStrict("update_career_settings", {
      input: {
        telemetryEnabled: getInputChecked("cmCareerTelemetryEnabledInput"),
        localStatsTrackingEnabled: getInputChecked("cmCareerLocalStatsTrackingInput"),
        autoJobLoggingEnabled: getInputChecked("cmCareerAutoJobLoggingInput"),
        autoFinanceTrackingEnabled: getInputChecked("cmCareerAutoFinanceTrackingInput"),
        useMetricUnits: getInputChecked("cmCareerMetricUnitsInput"),
        use24hTime: getInputChecked("cmCareer24hTimeInput"),
        autosaveCareerData: getInputChecked("cmCareerAutosaveInput"),
      },
    });

    await refreshAllData();
    showToast(await t("career_mode.career_settings_saved"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleAssignMemberRole() {
  try {
    const userId = Number(getInputValue("cmAssignUserIdInput"));
    const roleKey = getInputValue("cmAssignRoleSelect");
    if (!Number.isFinite(userId) || userId <= 0) {
      showToast(await t("career_mode.errors.user_not_found"), "error");
      return;
    }

    await invokeStrict("assign_member_role", {
      userId,
      roleKey,
    });

    await refreshAllData();
    showToast(await t("career_mode.member_role_saved"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function handleMemberRoleTableAction(event) {
  const button = event.target.closest("[data-member-role-apply]");
  if (!button) return;

  const memberId = Number(button.getAttribute("data-member-role-apply"));
  const select = document.querySelector(`[data-member-role-select=\"${memberId}\"]`);
  const roleKey = select?.value;
  if (!memberId || !roleKey) return;

  try {
    await invokeStrict("change_member_role", {
      memberId,
      roleKey,
    });
    await refreshAllData();
    showToast(await t("career_mode.member_role_saved"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
  }
}

async function initNavigation() {
  const openLauncherButton = document.getElementById("openLauncherBtn");
  const openSaveEditorButton = document.getElementById("openSaveEditorBtn");
  const refreshButton = document.getElementById("refreshCareerBtn");

  openLauncherButton?.addEventListener("click", async () => {
    await safeInvoke("hub_set_mode", { mode: "career" }, { silent: true });
    window.location.href = "/index.html";
  });

  openSaveEditorButton?.addEventListener("click", async () => {
    await safeInvoke("hub_set_mode", { mode: "editor" }, { silent: true });
    window.location.href = "/pages/save-editor/index.html";
  });

  refreshButton?.addEventListener("click", () => {
    void refreshAllData();
  });
}

function registerUserMenuHandlers() {
  document.getElementById("careerUserMenuBtn")?.addEventListener("click", (event) => {
    event.stopPropagation();
    toggleUserMenu();
  });

  document.getElementById("careerUserMenuLogin")?.addEventListener("click", async () => {
    setUserMenuOpen(false);
    await safeInvoke("hub_set_mode", { mode: "career" }, { silent: true });
    window.location.href = "/index.html";
  });

  document.getElementById("careerUserMenuProfile")?.addEventListener("click", () => {
    setUserMenuOpen(false);
    switchPanel("settings");
    document.getElementById("cmUserUsernameInput")?.focus();
  });

  document.getElementById("careerUserMenuSettings")?.addEventListener("click", () => {
    setUserMenuOpen(false);
    switchPanel("settings");
  });

  document.getElementById("careerUserMenuLogout")?.addEventListener("click", async () => {
    setUserMenuOpen(false);
    try {
      await invokeStrict("auth_logout");
    } catch (error) {
      showToast(await resolveErrorMessage(error), "error");
      return;
    }
    state.authUser = null;
    renderUserMenu();
  });

  document.addEventListener("click", (event) => {
    if (!event.target.closest(".career-user-menu")) {
      setUserMenuOpen(false);
    }
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      setUserMenuOpen(false);
    }
  });
}

function registerActionHandlers() {
  document.getElementById("cmUpdateUsernameBtn")?.addEventListener("click", () => {
    void handleUpdateUsername();
  });
  document.getElementById("cmSaveUserSettingsBtn")?.addEventListener("click", () => {
    void handleSaveUserSettings();
  });
  document.getElementById("cmCompanyProfileSaveBtn")?.addEventListener("click", () => {
    void handleSaveCompanyProfile();
  });
  document.getElementById("cmCompanySettingsSaveBtn")?.addEventListener("click", () => {
    void handleSaveCompanySettings();
  });
  document.getElementById("cmSaveCareerSettingsBtn")?.addEventListener("click", () => {
    void handleSaveCareerSettings();
  });
  document.getElementById("cmAssignRoleBtn")?.addEventListener("click", () => {
    void handleAssignMemberRole();
  });
  document.getElementById("cmMembersBody")?.addEventListener("click", (event) => {
    void handleMemberRoleTableAction(event);
  });
}

document.addEventListener("DOMContentLoaded", async () => {
  attachI18nToWindow();
  await translateDocument(document);
  await loadUiText();

  await safeInvoke("hub_set_mode", { mode: "career" }, { silent: true });

  applyNavHandlers();
  applyDispatcherHandlers();
  switchDispatcherTab("market");
  applySettingsHandlers();
  registerDispatcherEventHandlers();
  registerCareerEventHandlers();
  registerUserMenuHandlers();
  await initNavigation();
  registerActionHandlers();

  await refreshAllData();
  startRefreshLoop();
  startDispatcherClock();
});
