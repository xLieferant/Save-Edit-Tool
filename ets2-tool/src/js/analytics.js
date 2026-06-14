import { attachI18nToWindow, t, translateDocument } from "../modules/shared/i18n.js";
import { hasTauri, invoke, safeInvoke } from "../modules/shared/runtime.js";
import { exportAnalyticsCsv } from "./analytics-export.js";

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;")
    .replaceAll("'", "&#39;");
}

function formatNumber(value, digits = 0) {
  if (value == null || Number.isNaN(Number(value))) return "-";
  return Number(value).toLocaleString(undefined, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

function formatCurrency(value) {
  if (value == null) return "-";
  return `EUR ${formatNumber(value, 0)}`;
}

function formatDistance(value) {
  if (value == null) return "-";
  return `${formatNumber(value, 1)} km`;
}

function formatPercent(value) {
  if (value == null) return "-";
  return `${formatNumber(value, 1)}%`;
}

function formatDate(value) {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);
  return date.toLocaleString();
}

function normalizeSelectValue(value) {
  if (!value) return null;
  const trimmed = String(value).trim();
  if (!trimmed || trimmed === "all") return null;
  return trimmed;
}

function showToast(message, type = "info") {
  if (typeof window.showToast === "function") {
    window.showToast(message, type);
    return;
  }

  const toast = document.createElement("div");
  toast.className = "career-toast";
  toast.dataset.type = type;
  toast.textContent = message;
  document.body.appendChild(toast);
  setTimeout(() => {
    toast.remove();
  }, 3500);
}

async function loadCopy() {
  return {
    totalJobs: await t("career_mode.analytics.summary.total_jobs"),
    totalRevenue: await t("career_mode.analytics.summary.total_revenue"),
    totalCosts: await t("career_mode.analytics.summary.total_costs"),
    totalProfit: await t("career_mode.analytics.summary.total_profit"),
    totalDistance: await t("career_mode.analytics.summary.total_distance"),
    avgProfitJob: await t("career_mode.analytics.summary.avg_profit_job"),
    avgProfitKm: await t("career_mode.analytics.summary.avg_profit_km"),
    damagedDeliveries: await t("career_mode.analytics.summary.damaged_deliveries"),
    onTimeDeliveries: await t("career_mode.analytics.summary.on_time_deliveries"),
    lateDeliveries: await t("career_mode.analytics.summary.late_deliveries"),
    sourceLocalTracking: await t("career_mode.analytics.sources.local_tracking"),
    sourceSaveImport: await t("career_mode.analytics.sources.save_import"),
    sourcePartialSaveData: await t("career_mode.analytics.sources.partial_save_data"),
    sourceUnknown: await t("career_mode.analytics.sources.unknown"),
    noChartData: await t("career_mode.analytics.empty.no_chart_data"),
    noRows: await t("career_mode.analytics.empty.no_rows"),
    unknown: await t("career.shared.none"),
    trackingEnabled: await t("career_mode.analytics.messages.tracking_enabled"),
    scanSuccess: await t("career_mode.analytics.messages.scan_success"),
    scanFailed: await t("career_mode.analytics.messages.scan_failed"),
    exportSuccess: await t("career_mode.analytics.messages.export_success"),
    exportCanceled: await t("career_mode.analytics.messages.export_canceled"),
    exportFailed: await t("career_mode.analytics.messages.export_failed"),
    refreshFailed: await t("career_mode.analytics.messages.refresh_failed"),
    apiUnavailable: await t("career_mode.analytics.messages.api_unavailable"),
    partialData: await t("career_mode.analytics.partial_data"),
    note: await t("career_mode.analytics.note"),
    localTrackingHint: await t("career_mode.analytics.local_tracking_hint"),
    allOption: await t("career_mode.analytics.filters.all"),
    sourceLabel: await t("career_mode.analytics.table.source"),
    yesLabel: await t("career_mode.analytics.table.yes"),
    noLabel: await t("career_mode.analytics.table.no"),
    statusActive: await t("career_mode.analytics.status.active"),
    statusCompleted: await t("career_mode.analytics.status.completed"),
    statusFailed: await t("career_mode.analytics.status.failed"),
    statusCancelled: await t("career_mode.analytics.status.cancelled"),
    statusUnknown: await t("career_mode.analytics.status.unknown"),
  };
}

function resolveStatusLabel(copy, status) {
  switch (String(status || "").toLowerCase()) {
    case "active":
      return copy.statusActive;
    case "completed":
      return copy.statusCompleted;
    case "failed":
      return copy.statusFailed;
    case "cancelled":
      return copy.statusCancelled;
    default:
      return copy.statusUnknown;
  }
}

function resolveSourceLabel(copy, source) {
  switch (String(source || "").toLowerCase()) {
    case "local_tracking":
      return copy.sourceLocalTracking;
    case "save_import":
      return copy.sourceSaveImport;
    case "partial_save_data":
      return copy.sourcePartialSaveData;
    default:
      return copy.sourceUnknown;
  }
}

function resolveBooleanLabel(copy, value) {
  if (value == null) return "-";
  return value ? copy.yesLabel : copy.noLabel;
}

function getElements(root = document) {
  return {
    root,
    loading: root.querySelector("#analyticsLoading"),
    error: root.querySelector("#analyticsError"),
    errorText: root.querySelector("#analyticsErrorText"),
    empty: root.querySelector("#analyticsEmpty"),
    content: root.querySelector("#analyticsContent"),
    summaryGrid: root.querySelector("#analyticsSummaryGrid"),
    sourcePills: root.querySelector("#analyticsSourcePills"),
    dataHint: root.querySelector("#analyticsDataHint"),
    localHint: root.querySelector("#analyticsLocalHint"),
    refreshBtn: root.querySelector("#analyticsRefreshBtn"),
    exportBtn: root.querySelector("#analyticsExportBtn"),
    scanBtn: root.querySelector("#analyticsScanBtn"),
    enableTrackingBtn: root.querySelector("#analyticsEnableTrackingBtn"),
    emptyScanBtn: root.querySelector("#analyticsEmptyScanBtn"),
    emptyEnableBtn: root.querySelector("#analyticsEmptyEnableBtn"),
    filterFrom: root.querySelector("#analyticsFilterFrom"),
    filterTo: root.querySelector("#analyticsFilterTo"),
    filterProfile: root.querySelector("#analyticsFilterProfile"),
    filterGame: root.querySelector("#analyticsFilterGame"),
    filterStatus: root.querySelector("#analyticsFilterStatus"),
    filterCargo: root.querySelector("#analyticsFilterCargo"),
    filterSourceCity: root.querySelector("#analyticsFilterSourceCity"),
    filterDestinationCity: root.querySelector("#analyticsFilterDestinationCity"),
    applyBtn: root.querySelector("#analyticsApplyBtn"),
    resetBtn: root.querySelector("#analyticsResetBtn"),
    revenueChart: root.querySelector("#analyticsRevenueChart"),
    profitChart: root.querySelector("#analyticsProfitChart"),
    jobsChart: root.querySelector("#analyticsJobsChart"),
    cargoChart: root.querySelector("#analyticsCargoChart"),
    routesChart: root.querySelector("#analyticsRoutesChart"),
    damageChart: root.querySelector("#analyticsDamageChart"),
    historyBody: root.querySelector("#analyticsHistoryBody"),
  };
}

function readFilters(elements) {
  return {
    from: elements.filterFrom?.value?.trim() || null,
    to: elements.filterTo?.value?.trim() || null,
    profile: normalizeSelectValue(elements.filterProfile?.value),
    game: normalizeSelectValue(elements.filterGame?.value),
    status: normalizeSelectValue(elements.filterStatus?.value),
    cargo: elements.filterCargo?.value?.trim() || null,
    sourceCity: normalizeSelectValue(elements.filterSourceCity?.value),
    destinationCity: normalizeSelectValue(elements.filterDestinationCity?.value),
  };
}

function resetFilters(elements) {
  if (elements.filterFrom) elements.filterFrom.value = "";
  if (elements.filterTo) elements.filterTo.value = "";
  if (elements.filterProfile) elements.filterProfile.value = "all";
  if (elements.filterGame) elements.filterGame.value = "all";
  if (elements.filterStatus) elements.filterStatus.value = "all";
  if (elements.filterCargo) elements.filterCargo.value = "";
  if (elements.filterSourceCity) elements.filterSourceCity.value = "all";
  if (elements.filterDestinationCity) elements.filterDestinationCity.value = "all";
}

function setBusy(elements, busy) {
  [
    elements.refreshBtn,
    elements.exportBtn,
    elements.scanBtn,
    elements.emptyScanBtn,
    elements.enableTrackingBtn,
    elements.emptyEnableBtn,
    elements.applyBtn,
    elements.resetBtn,
  ].forEach((element) => {
    if (element) element.disabled = Boolean(busy);
  });
}

function showState(elements, state) {
  if (elements.loading) elements.loading.hidden = state !== "loading";
  if (elements.error) elements.error.hidden = state !== "error";
  if (elements.empty) elements.empty.hidden = state !== "empty";
  if (elements.content) elements.content.hidden = state !== "content";
}

function renderSummary(copy, elements, summary) {
  if (!elements.summaryGrid) return;

  const cards = [
    { label: copy.totalJobs, value: formatNumber(summary.totalJobs, 0) },
    { label: copy.totalRevenue, value: formatCurrency(summary.totalRevenue) },
    { label: copy.totalCosts, value: formatCurrency(summary.totalCosts) },
    { label: copy.totalProfit, value: formatCurrency(summary.totalProfit) },
    { label: copy.totalDistance, value: formatDistance(summary.totalDistanceKm) },
    { label: copy.avgProfitJob, value: summary.averageProfitPerJob == null ? "-" : formatCurrency(summary.averageProfitPerJob) },
    { label: copy.avgProfitKm, value: summary.averageProfitPerKm == null ? "-" : `EUR ${formatNumber(summary.averageProfitPerKm, 2)}` },
    { label: copy.damagedDeliveries, value: formatNumber(summary.damagedDeliveries, 0) },
    { label: copy.onTimeDeliveries, value: formatNumber(summary.onTimeDeliveries, 0) },
    { label: copy.lateDeliveries, value: formatNumber(summary.lateDeliveries, 0) },
  ];

  elements.summaryGrid.innerHTML = cards
    .map(
      (card) => `
        <article class="analytics-summary-card">
          <span>${escapeHtml(card.label)}</span>
          <strong>${escapeHtml(card.value)}</strong>
        </article>
      `
    )
    .join("");

  if (elements.sourcePills) {
    elements.sourcePills.innerHTML = [
      [copy.sourceLocalTracking, summary.sourceCounts?.localTracking],
      [copy.sourceSaveImport, summary.sourceCounts?.saveImport],
      [copy.sourcePartialSaveData, summary.sourceCounts?.partialSaveData],
      [copy.sourceUnknown, summary.sourceCounts?.unknown],
    ]
      .filter(([, value]) => Number(value || 0) > 0)
      .map(
        ([label, value]) => `
          <span class="analytics-source-pill">
            <strong>${escapeHtml(String(value))}</strong>
            <span>${escapeHtml(label)}</span>
          </span>
        `
      )
      .join("");
  }

  if (elements.dataHint) {
    elements.dataHint.textContent = summary.partialData ? copy.partialData : copy.note;
  }
  if (elements.localHint) {
    elements.localHint.textContent = copy.localTrackingHint;
  }
}

function renderChart(host, rows, copy, formatter = (value) => formatNumber(value, 0), secondaryFormatter = null) {
  if (!host) return;
  if (!Array.isArray(rows) || rows.length === 0) {
    host.innerHTML = `<div class="analytics-chart-empty">${escapeHtml(copy.noChartData)}</div>`;
    return;
  }

  const max = Math.max(1, ...rows.map((row) => Number(row.value || 0)));
  host.innerHTML = rows
    .map((row) => {
      const width = Math.max(6, Math.round((Number(row.value || 0) / max) * 100));
      const secondary = secondaryFormatter && row.valueSecondary != null
        ? `<small>${escapeHtml(secondaryFormatter(row.valueSecondary))}</small>`
        : "";
      return `
        <div class="analytics-bar-row">
          <div class="analytics-bar-copy">
            <strong>${escapeHtml(row.label)}</strong>
            <span>${escapeHtml(formatter(row.value))}</span>
            ${secondary}
          </div>
          <div class="analytics-bar-track" aria-hidden="true">
            <div class="analytics-bar-fill" style="width:${width}%"></div>
          </div>
        </div>
      `;
    })
    .join("");
}

function renderTable(copy, elements, items) {
  if (!elements.historyBody) return;
  if (!Array.isArray(items) || items.length === 0) {
    elements.historyBody.innerHTML = `
      <tr>
        <td colspan="22" class="analytics-table-empty">${escapeHtml(copy.noRows)}</td>
      </tr>
    `;
    return;
  }

  elements.historyBody.innerHTML = items
    .map((item) => {
      const profile = item.profileName || item.profileId || "-";
      return `
        <tr>
          <td>${escapeHtml(formatDate(item.startedAt || item.detectedAt))}</td>
          <td>${escapeHtml(profile)}</td>
          <td>${escapeHtml(item.game || "-")}</td>
          <td>${escapeHtml(item.jobUid)}</td>
          <td><span class="analytics-status-pill">${escapeHtml(resolveStatusLabel(copy, item.status))}</span></td>
          <td>${escapeHtml(item.cargoName || "-")}</td>
          <td>${escapeHtml(item.sourceCity || "-")}</td>
          <td>${escapeHtml(item.destinationCity || "-")}</td>
          <td>${escapeHtml(item.sourceCompany || "-")}</td>
          <td>${escapeHtml(item.destinationCompany || "-")}</td>
          <td>${escapeHtml(formatDistance(item.distanceKm))}</td>
          <td>${escapeHtml(formatCurrency(item.revenue))}</td>
          <td>${escapeHtml(formatCurrency(item.costs))}</td>
          <td>${escapeHtml(formatPercent(item.damagePercent))}</td>
          <td>${escapeHtml(formatCurrency(item.penalties))}</td>
          <td>${escapeHtml(formatCurrency(item.profit))}</td>
          <td>${escapeHtml(formatNumber(item.xp, 0))}</td>
          <td>${escapeHtml(formatNumber(item.levelAfter, 0))}</td>
          <td>${escapeHtml(item.truckName || "-")}</td>
          <td>${escapeHtml(item.trailerName || "-")}</td>
          <td>${escapeHtml(resolveBooleanLabel(copy, item.drivenWithTruck))}</td>
          <td>${escapeHtml(resolveSourceLabel(copy, item.source))}</td>
        </tr>
      `;
    })
    .join("");
}

function renderFilterOptions(copy, elements, filterOptions) {
  const selections = {
    profile: elements.filterProfile?.value || "all",
    sourceCity: elements.filterSourceCity?.value || "all",
    destinationCity: elements.filterDestinationCity?.value || "all",
  };

    const updateSelect = (element, rows) => {
    if (!element) return;
    element.innerHTML = [
      `<option value="all">${escapeHtml(copy.allOption)}</option>`,
      ...rows.map((row) => `<option value="${escapeHtml(row)}">${escapeHtml(row)}</option>`),
    ].join("");
  };

  updateSelect(elements.filterProfile, filterOptions?.profiles || []);
  updateSelect(elements.filterSourceCity, filterOptions?.sourceCities || []);
  updateSelect(elements.filterDestinationCity, filterOptions?.destinationCities || []);

  if (elements.filterProfile) elements.filterProfile.value = selections.profile;
  if (elements.filterSourceCity) elements.filterSourceCity.value = selections.sourceCity;
  if (elements.filterDestinationCity) elements.filterDestinationCity.value = selections.destinationCity;
}

export function createAnalyticsController(options = {}) {
  const elements = getElements(options.root || document);
  const state = {
    copy: null,
    summary: null,
    history: null,
    careerSettings: null,
  };

  async function refresh() {
    if (!hasTauri) {
      showState(elements, "error");
      if (elements.errorText) {
        elements.errorText.textContent = state.copy?.apiUnavailable || state.copy?.refreshFailed || "Analytics unavailable.";
      }
      return;
    }

    setBusy(elements, true);
    showState(elements, "loading");

    const filters = readFilters(elements);
    const [summary, history, careerSettings] = await Promise.all([
      safeInvoke("career_get_analytics_summary", { filters }, { fallback: null, silent: true }),
      safeInvoke("career_get_analytics_job_history", { filters }, { fallback: null, silent: true }),
      safeInvoke("get_career_settings", {}, { fallback: null, silent: true }),
    ]);

    if (!summary || !history) {
      showState(elements, "error");
      if (elements.errorText) {
        elements.errorText.textContent = state.copy?.refreshFailed || "Analytics refresh failed.";
      }
      setBusy(elements, false);
      return;
    }

    state.summary = summary;
    state.history = history;
    state.careerSettings = careerSettings;

    renderFilterOptions(state.copy, elements, history.filterOptions);
    renderSummary(state.copy, elements, summary);
    renderChart(elements.revenueChart, history.charts?.revenueOverTime, state.copy, (value) => formatCurrency(value));
    renderChart(elements.profitChart, history.charts?.profitOverTime, state.copy, (value) => formatCurrency(value));
    renderChart(elements.jobsChart, history.charts?.jobsPerDay, state.copy, (value) => formatNumber(value, 0));
    renderChart(elements.cargoChart, history.charts?.topCargo, state.copy, (value) => formatNumber(value, 0));
    renderChart(elements.routesChart, history.charts?.topRoutes, state.copy, (value) => formatNumber(value, 0));
    renderChart(
      elements.damageChart,
      history.charts?.damageCostAnalysis,
      state.copy,
      (value) => formatPercent(value),
      (value) => formatCurrency(value)
    );
    renderTable(state.copy, elements, history.items || []);

    showState(elements, history.totalFilteredJobs > 0 ? "content" : "empty");
    setBusy(elements, false);
  }

  async function handleScan() {
    setBusy(elements, true);
    const result = await safeInvoke(
      "career_scan_profile_job_history",
      { profilePath: null },
      { fallback: null, silent: true }
    );
    setBusy(elements, false);

    if (!result) {
      showToast(state.copy.scanFailed, "error");
      return;
    }

    showToast(
      `${state.copy.scanSuccess} ${result.detectedJobs} / ${result.scannedSaves}`,
      "success"
    );
    await refresh();
  }

  async function handleEnableTracking() {
    const settings = state.careerSettings
      || await safeInvoke("get_career_settings", {}, { fallback: null, silent: true });
    if (!settings) {
      showToast(state.copy.refreshFailed, "error");
      return;
    }

    try {
      await invoke("update_career_settings", {
        input: {
          telemetryEnabled: settings.telemetryEnabled ?? true,
          localStatsTrackingEnabled: true,
          autoJobLoggingEnabled: true,
          autoFinanceTrackingEnabled: settings.autoFinanceTrackingEnabled ?? true,
          useMetricUnits: settings.useMetricUnits ?? true,
          use24hTime: settings.use24hTime ?? true,
          autosaveCareerData: settings.autosaveCareerData ?? true,
        },
      });
      showToast(state.copy.trackingEnabled, "success");
      await refresh();
    } catch (error) {
      console.error("analytics tracking enable failed", error);
      showToast(state.copy.refreshFailed, "error");
    }
  }

  async function handleExport() {
    try {
      const path = await exportAnalyticsCsv(readFilters(elements));
      if (!path) {
        showToast(state.copy.exportCanceled, "warning");
        return;
      }
      showToast(`${state.copy.exportSuccess} ${path}`, "success");
    } catch (error) {
      console.error("analytics export failed", error);
      showToast(state.copy.exportFailed, "error");
    }
  }

  function bindEvents() {
    elements.refreshBtn?.addEventListener("click", () => {
      void refresh();
    });
    elements.applyBtn?.addEventListener("click", () => {
      void refresh();
    });
    elements.resetBtn?.addEventListener("click", () => {
      resetFilters(elements);
      void refresh();
    });
    elements.scanBtn?.addEventListener("click", () => {
      void handleScan();
    });
    elements.emptyScanBtn?.addEventListener("click", () => {
      void handleScan();
    });
    elements.enableTrackingBtn?.addEventListener("click", () => {
      void handleEnableTracking();
    });
    elements.emptyEnableBtn?.addEventListener("click", () => {
      void handleEnableTracking();
    });
    elements.exportBtn?.addEventListener("click", () => {
      void handleExport();
    });
  }

  return {
    async init() {
      state.copy = await loadCopy();
      bindEvents();
      await refresh();
    },
    refresh,
  };
}

export async function initStandaloneAnalyticsPage() {
  attachI18nToWindow();
  await translateDocument(document);
  const controller = createAnalyticsController();
  await controller.init();
}
