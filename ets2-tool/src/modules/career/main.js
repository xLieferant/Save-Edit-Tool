import { attachI18nToWindow, t, translateDocument } from "../shared/i18n.js";
import { safeInvoke } from "../shared/runtime.js";

const state = {
  overview: null,
  status: null,
  jobLog: [],
  jobStats: null,
  source: "mock",
};

const uiText = {
  online: "Online",
  offline: "Offline",
  noData: "-",
  sourceLive: "Live",
  sourceMock: "Mock",
  companyLiveDesc: "-",
  companyMockDesc: "-",
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

function getFallbackOverview() {
  const now = Date.now();
  return {
    economy: {
      companyName: "Alex Logistics",
      pricePerKm: 24.5,
      dieselPricePerLiter: 1.8,
      tollPerKm: 0.2,
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
    employees: [
      { name: "Alex", role: "Owner", status: "On Duty", location: "Berlin" },
      { name: "Lena", role: "Driver", status: "Resting", location: "Hamburg" },
      { name: "Marco", role: "Driver", status: "On Duty", location: "Prague" },
    ],
    jobs: [
      {
        id: "JOB-001",
        origin: "Berlin",
        destination: "Prague",
        cargo: "Medical Supplies",
        payout: 18400,
        deliveryEtaHours: 7,
      },
    ],
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
    fleetOverview: {
      trucks: 3,
      trailers: 2,
      avgCondition: 91,
    },
    currentJob: {
      id: "JOB-001",
      origin: "Berlin",
      destination: "Prague",
      cargo: "Medical Supplies",
      payout: 18400,
      deliveryEtaHours: 7,
    },
    activeJob: {
      jobId: "JOB-001",
      originCity: "Berlin",
      destinationCity: "Prague",
      cargo: "Medical Supplies",
      income: 18400,
      remainingTimeMin: 318,
    },
    activeTrip: {
      origin: "Berlin",
      destination: "Prague",
      distanceKm: 350,
      durationSeconds: 23000,
    },
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
  };
}

function getSourceLabel(source) {
  return source === "live" ? uiText.sourceLive : uiText.sourceMock;
}

function statusLabel(active) {
  return active ? uiText.online : uiText.offline;
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

function renderDashboard() {
  const overview = state.overview;
  if (!overview) return;

  const companyName = overview.economy?.companyName || uiText.noData;
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
    lastTrip
      ? `${lastTrip.originCity || "-"} -> ${lastTrip.destinationCity || "-"}`
      : uiText.noData
  );

  const overviewList = document.getElementById("cmQuickOverviewList");
  if (overviewList) {
    const lines = [
      `${companyName}`,
      `${formatCurrency(balance)}`,
      `${formatNumber(overview.fleetOverview?.trucks || 0)} Trucks | ${formatNumber(overview.fleetOverview?.trailers || 0)} Trailers`,
      `${formatNumber(overview.jobStats?.totalJobs || 0)} Jobs | ${Math.round(Number(overview.jobStats?.successRate || 0) * 100)}% Success`,
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
  const employees = Array.isArray(state.overview?.employees) ? state.overview.employees : [];

  const rows = employees.map((member, index) => {
    const name = member.name || member.username || `Driver ${index + 1}`;
    const role = member.role || "Driver";
    const status = member.status || "-";
    const location = member.location || member.city || "-";

    return `
      <tr>
        <td>${name}</td>
        <td>${role}</td>
        <td>${status}</td>
        <td>${location}</td>
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
        <td>${formatDate(job.startedAtUtc || job.startedAt || job.startedAtUtc)}</td>
        <td>${job.cargo || "-"}</td>
        <td>${route}</td>
        <td>${formatDistance(job.plannedDistanceKm || job.distanceKm || 0)}</td>
        <td>${formatCurrency(job.income || 0)}</td>
      </tr>
    `;
  });

  setTableRows("cmOrderHistoryBody", historyRows, uiText.noData, 5);
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
  const economy = state.overview?.economy || {};

  setCellText("cmCompanyFieldName", economy.companyName || uiText.noData);
  setCellText("cmCompanyFieldLocation", state.source === "live" ? "-" : "Berlin");
  setCellText("cmCompanyFieldLanguage", state.source === "live" ? "-" : "EN");
  setCellText("cmCompanyFieldGame", state.status?.active_game || state.status?.activeGame || "ETS2");
  setCellText(
    "cmCompanyFieldDescription",
    state.source === "live"
      ? uiText.companyLiveDesc
      : uiText.companyMockDesc
  );
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

  setCellText("careerStatusGameRunning", statusLabel(gameRunning));
  setCellText("careerStatusPluginInstalled", statusLabel(pluginInstalled));
  setCellText("careerStatusBridgeConnected", statusLabel(bridgeConnected));
  setCellText("careerStatusActiveGame", activeGame);
  setCellText("careerDataSource", getSourceLabel(state.source));
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
    const enabled = document.getElementById("cmSettingAutoRefresh")?.checked;
    if (enabled) {
      void refreshCareerData();
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
}

async function refreshCareerData() {
  const status = await safeInvoke("career_get_status", {}, { fallback: null, silent: true });
  const overview = await safeInvoke("career_get_overview", {}, { fallback: null, silent: true });
  const jobLog = await safeInvoke("career_get_job_log", {}, { fallback: [], silent: true });
  const jobStats = await safeInvoke("career_get_job_stats", {}, { fallback: null, silent: true });

  const allowMock = document.getElementById("cmSettingMockFallback")?.checked ?? true;
  const hasLiveOverview = Boolean(overview);

  state.status = status || null;
  state.overview = hasLiveOverview ? overview : allowMock ? getFallbackOverview() : { ...getFallbackOverview(), recentJobs: [] };
  state.jobLog = Array.isArray(jobLog) && jobLog.length > 0 ? jobLog : state.overview.recentJobs || [];
  state.jobStats = jobStats || state.overview.jobStats || null;
  state.source = hasLiveOverview ? "live" : "mock";

  if (state.jobStats && state.overview) {
    state.overview.jobStats = state.jobStats;
  }

  renderStatus();
  renderDashboard();
  renderMembers();
  renderOrders();
  renderFinances();
  renderFleet();
  renderCompany();
  renderStatistics();
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
    void refreshCareerData();
  });
}

document.addEventListener("DOMContentLoaded", async () => {
  attachI18nToWindow();
  await translateDocument(document);
  await loadUiText();

  await safeInvoke("hub_set_mode", { mode: "career" }, { silent: true });

  applyNavHandlers();
  applySettingsHandlers();
  await initNavigation();
  await refreshCareerData();
  startRefreshLoop();
});
