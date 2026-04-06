import { attachI18nToWindow, t, translateDocument } from "../shared/i18n.js";
import { hasTauri, invoke, safeInvoke } from "../shared/runtime.js";

const state = {
  overview: null,
  status: null,
  jobLog: [],
  jobStats: null,
  source: "mock",
  userProfile: null,
  userSettings: null,
  companyOverview: null,
  companyMembers: [],
  companySettings: null,
  careerSettings: null,
  roles: [],
  dispatcher: {
    overview: null,
    marketJobs: [],
    selectedJobId: null,
    selectedJob: null,
    activeJobs: [],
    history: null,
    contacts: [],
    offers: [],
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
  const raw = String(rawError || "");
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
  ];

  for (const code of knownCodes) {
    if (raw.includes(code)) return code;
  }

  return null;
}

async function resolveErrorMessage(rawError) {
  const code = normalizeErrorCode(rawError);
  if (!code) {
    return `${uiText.errorPrefix}: ${String(rawError || "unknown")}`;
  }

  const translationKey = `career_mode.errors.${code}`;
  const translated = await t(translationKey);
  if (translated === translationKey) {
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
    return `
      <tr>
        <td>${job.cargo || "-"}</td>
        <td>${route}</td>
        <td>${formatCurrency(job.income || 0)}</td>
        <td>${job.status || "-"}</td>
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

    activeRows.push(`
      <tr>
        <td>${activeJob.jobId || activeJob.id || "-"}</td>
        <td>${route}</td>
        <td>${activeJob.cargo || "-"}</td>
        <td>${formatCurrency(activeJob.income || activeJob.payout || 0)}</td>
        <td>${remaining}</td>
      </tr>
    `);
  }

  setTableRows("cmActiveOrdersBody", activeRows, uiText.noData, 5);

  const historyRows = (state.jobLog || []).slice(0, 20).map((job) => {
    const route = `${job.originCity || "-"} -> ${job.destinationCity || "-"}`;
    return `
      <tr>
        <td>${formatDate(job.startedAtUtc || job.startedAt || job.started_at_utc)}</td>
        <td>${job.cargo || "-"}</td>
        <td>${route}</td>
        <td>${formatDistance(job.plannedDistanceKm || job.planned_distance_km || 0)}</td>
        <td>${formatCurrency(job.income || 0)}</td>
      </tr>
    `;
  });

  setTableRows("cmOrderHistoryBody", historyRows, uiText.noData, 5);
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

function renderDispatcherOverview() {
  const overview = state.dispatcher.overview || {};
  setCellText("cmDispatcherOpenJobs", formatNumber(overview.openMarketJobs || 0, 0));
  setCellText("cmDispatcherActiveJobs", formatNumber(overview.activeJobs || 0, 0));
  setCellText("cmDispatcherOpenOffers", formatNumber(overview.openOffers || 0, 0));
  setCellText("cmDispatcherContracts", formatNumber(overview.acceptedContracts || 0, 0));
}

function renderDispatcherMarket() {
  const rows = (state.dispatcher.marketJobs || []).map((job) => {
    const route = `${job.originCity || "-"} -> ${job.destinationCity || "-"}`;
    const selectedClass = job.id === state.dispatcher.selectedJobId ? "is-selected" : "";
    return `
      <tr class="${selectedClass}" data-dispatcher-job-id="${job.id}">
        <td>${job.id || "-"}</td>
        <td>${job.companyName || "-"}</td>
        <td>${job.jobType || "-"}</td>
        <td>${route}</td>
        <td>${formatDistance(job.distanceKm || 0)}</td>
        <td>${formatNumber(job.calculatedRatePerKm || 0, 2)}</td>
        <td>${formatCurrency(job.totalReward || 0)}</td>
        <td>${job.status || "-"}</td>
      </tr>
    `;
  });
  setTableRows("cmDispatcherMarketBody", rows, uiText.noData, 8);
}

function renderDispatcherDetails() {
  const selected = state.dispatcher.selectedJob;
  if (!selected) {
    setCellText("cmDispatcherDetailId", "-");
    setCellText("cmDispatcherDetailCompany", "-");
    setCellText("cmDispatcherDetailType", "-");
    setCellText("cmDispatcherDetailRoute", "-");
    setCellText("cmDispatcherDetailDistance", "-");
    setCellText("cmDispatcherDetailRate", "-");
    setCellText("cmDispatcherDetailReward", "-");
    setCellText("cmDispatcherDetailStatus", "-");
    setCellText("cmDispatcherDetailTier", "-");
    setCellText("cmDispatcherDetailReputation", "-");
    setCellText("cmDispatcherDetailProfit", "-");
    setCellText("cmDispatcherDetailBonus", "-");
    setCellText("cmDispatcherDetailRisk", "-");
    return;
  }

  const job = selected.job || selected;
  const route = `${job.originCity || "-"} -> ${job.destinationCity || "-"}`;
  setCellText("cmDispatcherDetailId", job.id || "-");
  setCellText("cmDispatcherDetailCompany", job.companyName || "-");
  setCellText("cmDispatcherDetailType", job.jobType || "-");
  setCellText("cmDispatcherDetailRoute", route);
  setCellText("cmDispatcherDetailDistance", formatDistance(job.distanceKm || 0));
  setCellText("cmDispatcherDetailRate", formatNumber(job.calculatedRatePerKm || 0, 2));
  setCellText("cmDispatcherDetailReward", formatCurrency(job.totalReward || 0));
  setCellText("cmDispatcherDetailStatus", job.status || "-");
  setCellText("cmDispatcherDetailTier", job.paymentTierSnapshot || "-");
  setCellText("cmDispatcherDetailReputation", formatNumber(job.companyReputation || 0, 0));
  setCellText("cmDispatcherDetailProfit", formatCurrency(job.profitEstimate || 0));
  setCellText("cmDispatcherDetailBonus", job.bonusNote || "-");
  setCellText("cmDispatcherDetailRisk", job.riskNote || "-");
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
    const progress = `${formatDistance(job.progressKm || 0)} / ${formatDistance(job.distanceKm || 0)}`;
    return `
      <tr>
        <td>${job.id || "-"}</td>
        <td>${job.companyName || "-"}</td>
        <td>${job.jobType || "-"}</td>
        <td>${route}</td>
        <td>${job.status || "-"}</td>
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
    return `
      <tr>
        <td>${job.id || "-"}</td>
        <td>${job.companyName || "-"}</td>
        <td>${job.jobType || "-"}</td>
        <td>${route}</td>
        <td>${formatCurrency(job.totalReward || 0)}</td>
        <td>${job.status || "-"}</td>
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
  state.dispatcher.overview = await safeInvoke("dispatcher_get_dispatcher_overview", {}, { fallback: null, silent: true });
  state.dispatcher.marketJobs = await safeInvoke(
    "dispatcher_get_market_jobs",
    { filter },
    { fallback: [], silent: true }
  );
  state.dispatcher.activeJobs = await safeInvoke("dispatcher_get_active_jobs", {}, { fallback: [], silent: true });
  state.dispatcher.history = await safeInvoke("dispatcher_get_job_history", {}, { fallback: { summary: {}, items: [] }, silent: true });
  state.dispatcher.contacts = await safeInvoke("dispatcher_get_company_contacts", {}, { fallback: [], silent: true });
  state.dispatcher.offers = await safeInvoke("dispatcher_get_offers", {}, { fallback: [], silent: true });

  if (state.dispatcher.selectedJobId) {
    state.dispatcher.selectedJob = await safeInvoke(
      "dispatcher_get_job_details",
      { jobId: state.dispatcher.selectedJobId },
      { fallback: null, silent: true }
    );
  } else if (state.dispatcher.marketJobs.length > 0) {
    state.dispatcher.selectedJobId = state.dispatcher.marketJobs[0].id;
    state.dispatcher.selectedJob = await safeInvoke(
      "dispatcher_get_job_details",
      { jobId: state.dispatcher.selectedJobId },
      { fallback: null, silent: true }
    );
  } else {
    state.dispatcher.selectedJob = null;
  }
}

async function handleDispatcherSelectJob(event) {
  const row = event.target.closest("[data-dispatcher-job-id]");
  if (!row) return;
  state.dispatcher.selectedJobId = row.getAttribute("data-dispatcher-job-id");
  state.dispatcher.selectedJob = await safeInvoke(
    "dispatcher_get_job_details",
    { jobId: state.dispatcher.selectedJobId },
    { fallback: null, silent: true }
  );
  renderDispatcherMarket();
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
  try {
    const details = await invokeStrict("dispatcher_accept_job", { jobId: state.dispatcher.selectedJobId });
    state.dispatcher.selectedJob = details;
    await refreshDispatcherData();
    renderDispatcher();
    showToast(await t("career_mode.dispatcher.toast_job_accepted"), "success");
  } catch (error) {
    showToast(await resolveErrorMessage(error), "error");
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

  setCellText("cmStatTrips", formatNumber(statistics.completedTrips || 0, 0));
  setCellText("cmStatKm", formatNumber(statistics.totalKilometers || 0, 0));
  setCellText("cmStatRevenue", formatCurrency(statistics.totalIncome || 0));
  setCellText("cmStatCompanyValue", formatCurrency(statistics.companyValue || 0));
  setCellText("cmStatSpeed", `${formatNumber(statistics.averageSpeed || 0, 1)} km/h`);
  setCellText("cmStatSpeeding", formatNumber(statistics.speedingEvents || 0, 0));

  const bars = [
    Number(statistics.completedTrips || 0),
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
  document.getElementById("cmDispatcherApplyFiltersBtn")?.addEventListener("click", () => {
    void handleDispatcherApplyFilters();
  });
  document.getElementById("cmDispatcherResetFiltersBtn")?.addEventListener("click", () => {
    void handleDispatcherResetFilters();
  });
  document.getElementById("cmDispatcherAcceptJobBtn")?.addEventListener("click", () => {
    void handleDispatcherAcceptJob();
  });
  document.getElementById("cmDispatcherSendOfferBtn")?.addEventListener("click", () => {
    void handleDispatcherSendOffer();
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
  state.jobLog = Array.isArray(jobLog) && jobLog.length > 0 ? jobLog : state.overview.recentJobs || [];
  state.jobStats = jobStats || state.overview.jobStats || null;
  state.source = hasLiveOverview ? "live" : "mock";

  if (state.jobStats && state.overview) {
    state.overview.jobStats = state.jobStats;
  }
}

async function refreshManagementData() {
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
  await initNavigation();
  registerActionHandlers();

  await refreshAllData();
  startRefreshLoop();
});
