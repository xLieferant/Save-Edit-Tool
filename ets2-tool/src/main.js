import { loadTools, activeTab, openCloneProfileModal, openModalMulti, openModalText } from "./app.js";
import { updateToolImagesForGame } from "./tools.js";
import { applySetting } from "./js/applySetting.js";
import { checkUpdaterOnStartup, manualUpdateCheck } from "./js/updater.js";

const { app } = window.__TAURI__;
const { openUrl } = window.__TAURI__.opener;
const { invoke, convertFileSrc } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let lastSelectedGame = null;
window.invoke = invoke;
window.applySetting = applySetting;

function formatTelemetryNumber(value, digits = 0) {
  return Number(value ?? 0).toLocaleString(undefined, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

function formatCurrency(value) {
  return `EUR ${formatTelemetryNumber(value ?? 0, 0)}`;
}

function deriveLevel(xp) {
  return Math.max(1, Math.floor(Number(xp ?? 0) / 1500) + 1);
}

function getThemeFallbackIcon() {
  return document.body.classList.contains("theme-light")
    ? "images/icon_Black.png"
    : "images/icon_White.png";
}

function resolveProfileIcon(profile) {
  if (profile?.avatar) {
    return profile.avatar.startsWith("data:")
      ? profile.avatar
      : convertFileSrc(profile.avatar);
  }
  return getThemeFallbackIcon();
}

function handleIconError(img) {
  img.onerror = null;
  img.src = getThemeFallbackIcon();
  img.removeAttribute("data-has-avatar");
}

function updateAllProfileIcons() {
  const fallback = getThemeFallbackIcon();
  document.querySelectorAll(".profile-icon-dropdown").forEach((img) => {
    if (!img.dataset.hasAvatar) {
      img.src = fallback;
    }
  });
  if (!window.selectedProfileHasAvatar) {
    document
      .querySelectorAll("#activeProfileIcon, .nav-icon-profile")
      .forEach((img) => (img.src = fallback));
  }
}

new MutationObserver(updateAllProfileIcons).observe(document.body, {
  attributes: true,
  attributeFilter: ["class"],
});

async function t(key, params = {}) {
  try {
    let text = await invoke("translate_command", { key });
    for (const [k, v] of Object.entries(params)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
    return text;
  } catch (error) {
    console.error("Translation error:", error);
    return key;
  }
}

async function translateUI() {
  const elements = document.querySelectorAll("[data-translate]");
  for (const el of elements) {
    const key = el.getAttribute("data-translate");
    el.textContent = await t(key);
  }

  const placeholders = document.querySelectorAll("[data-translate-placeholder]");
  for (const el of placeholders) {
    const key = el.getAttribute("data-translate-placeholder");
    el.setAttribute("placeholder", await t(key));
  }
}

window.t = t;
window.translateUI = translateUI;

window.showToast = async function (messageOrKey, options = {}, type = "info") {
  const resolvedOptions = typeof options === "string" ? {} : options;
  const resolvedType = typeof options === "string" ? options : type;
  const message = await t(messageOrKey, resolvedOptions);
  const toast = document.createElement("div");
  toast.className = `toast toast-${resolvedType}`;
  toast.innerHTML = `<span class="toast-icon">${resolvedType === "success" ? "OK" : resolvedType === "error" ? "ER" : resolvedType === "warning" ? "!" : "i"}</span><span class="toast-text"></span>`;
  toast.querySelector(".toast-text").textContent = message;
  document.body.appendChild(toast);
  requestAnimationFrame(() => toast.classList.add("show"));
  setTimeout(() => {
    toast.classList.remove("show");
    setTimeout(() => toast.remove(), 300);
  }, 4500);
};

async function appVersion() {
  try {
    return await app.getVersion();
  } catch (error) {
    console.error("Version load failed:", error);
    return "0.0.0";
  }
}

async function logUserAction(action, stage = "start") {
  try {
    await invoke("log_user_action", { action, stage });
  } catch (error) {
    console.warn("User log failed:", error);
  }
}

window.logUserAction = logUserAction;

document.addEventListener("DOMContentLoaded", async () => {
  await translateUI();
  document.body.classList.add("mode-editor");

  const refs = {
    hubScreen: document.getElementById("hubScreen"),
    hubHomeBtn: document.getElementById("hubHomeBtn"),
    hubCareerCard: document.getElementById("hubCareerCard"),
    hubEditorCard: document.getElementById("hubEditorCard"),
    profileStatus: document.getElementById("profile-status"),
    profileNameDisplay: document.getElementById("profileNameDisplay"),
    profileDropdownList: document.getElementById("profileDropdownList"),
    saveNameDisplay: document.getElementById("saveName"),
    saveDropdownList: document.getElementById("saveDropdownList"),
    openSaveModalBtn: document.getElementById("openSaveModal"),
    ets2Btn: document.getElementById("ets2Btn"),
    atsBtn: document.getElementById("atsBtn"),
    editorModeBtn: document.getElementById("editorModeBtn"),
    careerModeBtn: document.getElementById("careerModeBtn"),
    saveSafeModeBtn: document.getElementById("saveSafeModeBtn"),
    saveAdvancedModeBtn: document.getElementById("saveAdvancedModeBtn"),
    editorModeNotice: document.getElementById("editorModeNotice"),
    editorProfileValue: document.getElementById("editorProfileValue"),
    editorSaveValue: document.getElementById("editorSaveValue"),
    editorMoneyValue: document.getElementById("editorMoneyValue"),
    editorXpValue: document.getElementById("editorXpValue"),
    editorLevelValue: document.getElementById("editorLevelValue"),
    editorFleetValue: document.getElementById("editorFleetValue"),
    editorStageTitle: document.getElementById("editorStageTitle"),
    editorStageSummary: document.getElementById("editorStageSummary"),
    statusGameRunning: document.getElementById("statusGameRunning"),
    statusPluginInstalled: document.getElementById("statusPluginInstalled"),
    statusSdkConnected: document.getElementById("statusSdkConnected"),
    careerSidebarBalance: document.getElementById("careerSidebarBalance"),
    careerSidebarCompany: document.getElementById("careerSidebarCompany"),
    careerDashboardShell: document.querySelector(".career-dashboard-shell"),
    careerDetailHost: document.getElementById("careerDetailHost"),
    careerHeroTitle: document.getElementById("careerHeroTitle"),
    careerGameLabel: document.getElementById("careerGameLabel"),
    careerConnectionNote: document.getElementById("careerConnectionNote"),
    careerCompanyValue: document.getElementById("careerCompanyValue"),
    careerBalanceValue: document.getElementById("careerBalanceValue"),
    careerReputationValue: document.getElementById("careerReputationValue"),
    careerFleetStatusValue: document.getElementById("careerFleetStatusValue"),
    careerSpeedDial: document.getElementById("careerSpeedDial"),
    careerSpeedValue: document.getElementById("careerSpeedValue"),
    careerGearValue: document.getElementById("careerGearValue"),
    careerFuelValue: document.getElementById("careerFuelValue"),
    careerFuelPercent: document.getElementById("careerFuelPercent"),
    careerFuelBarFill: document.getElementById("careerFuelBarFill"),
    careerRpmValue: document.getElementById("careerRpmValue"),
    careerLiveRevenueValue: document.getElementById("careerLiveRevenueValue"),
    careerCostFuelValue: document.getElementById("careerCostFuelValue"),
    careerCostRepairValue: document.getElementById("careerCostRepairValue"),
    careerCostTollValue: document.getElementById("careerCostTollValue"),
    careerDriversOnlineValue: document.getElementById("careerDriversOnlineValue"),
    careerDriversRestingValue: document.getElementById("careerDriversRestingValue"),
    versionBtn: document.getElementById("versionBtn"),
    websiteBtn: document.getElementById("websiteBtn"),
    youtubeBtn: document.getElementById("youtubeBtn"),
    patreonBtn: document.getElementById("patreonBtn"),
    githubBtn: document.getElementById("githubBtn"),
    refreshBtn: document.getElementById("refreshBtn"),
    cloneBtn: document.getElementById("cloneProfileBtn"),
  };

  const careerText = {
    waiting: await t("career.status.awaiting_signal"),
    live: await t("career.status.sdk_live"),
    paused: await t("career.status.sdk_paused"),
    missingPlugin: await t("career.status.plugin_missing"),
    gameStopped: await t("career.status.game_stopped"),
  };
  const careerState = {
    gameRunning: false,
    pluginInstalled: false,
    bridgeConnected: false,
    paused: false,
    activeGame: "ets2",
  };
  const uiText = {
    noProfile: await t("editor.no_profile"),
    noSave: await t("editor.no_save"),
    noCompany: await t("career.hero.no_company"),
    loadingProfile: await t("status_text.loading_profile"),
    loadingSave: await t("status_text.loading_save"),
    profileLoaded: await t("status_text.profile_loaded"),
    saveLoaded: await t("status_text.save_loaded"),
    profileError: await t("status_text.profile_error"),
    scanningProfiles: await t("status_text.scanning_profiles"),
    scanFailed: await t("status_text.scan_failed"),
    safeMode: await t("editor.safe_desc"),
    advancedMode: await t("editor.advanced_desc"),
    pluginOnline: await t("career.plugin.connected"),
    pluginOffline: await t("career.plugin.disconnected"),
    pluginInstalledDetail: await t("career.plugin.installed_detail"),
    pluginMissingDetail: await t("career.plugin.missing_detail"),
    bridgeOnline: await t("career.plugin.bridge_online"),
    bridgeOffline: await t("career.plugin.bridge_offline"),
    bridgeOnlineDetail: await t("career.plugin.bridge_online_detail"),
    bridgeOfflineDetail: await t("career.plugin.bridge_offline_detail"),
  };
  const editorStageMeta = {
    truck: {
      title: "editor.stage.truck",
      summary: "editor.stage.truck_desc",
    },
    trailer: {
      title: "editor.stage.trailer",
      summary: "editor.stage.trailer_desc",
    },
    profile: {
      title: "editor.stage.profile",
      summary: "editor.stage.profile_desc",
    },
    settings: {
      title: "editor.stage.settings",
      summary: "editor.stage.settings_desc",
    },
  };
  const careerNavButtons = Array.from(document.querySelectorAll(".career-nav-btn"));

  const buildSectionFrame = async (eyebrowKey, titleKey, summaryKey, bodyMarkup) => `
    <div class="section-head">
      <div>
        <span class="eyebrow">${await t(eyebrowKey)}</span>
        <h2>${await t(titleKey)}</h2>
      </div>
      <p>${await t(summaryKey)}</p>
    </div>
    ${bodyMarkup}
  `;

  const buildDetailCards = (items, columns = "three-up") => `
    <div class="detail-grid ${columns}">
      ${items.map((item) => `
        <article class="detail-card">
          <span>${item.label}</span>
          <strong>${item.value}</strong>
          ${item.copy ? `<p>${item.copy}</p>` : ""}
        </article>
      `).join("")}
    </div>
  `;

  const renderCareerDetailPanel = async (panel) => {
    const money = Number(window.currentProfileData?.money ?? 0);
    const xp = Number(window.currentProfileData?.xp ?? 0);
    const level = deriveLevel(xp);
    const companyName = refs.profileNameDisplay?.textContent?.trim() || uiText.noCompany;
    const truckCount = window.allTrucks?.length || 0;
    const trailerCount = window.allTrailers?.length || 0;

    switch (panel) {
      case "members":
        return buildSectionFrame(
          "career.nav.members",
          "career.members.title",
          "career.members.summary",
          buildDetailCards([
            { label: await t("career.members.lead_driver"), value: "Elena Hoffmann", copy: "Level 24 / ADR specialist" },
            { label: await t("career.members.dispatchers"), value: "03", copy: "Routing coverage across EU corridors" },
            { label: await t("career.members.recruiters"), value: "02", copy: "Expansion pipeline ready" },
          ])
        );
      case "orders":
        return buildSectionFrame(
          "career.nav.orders",
          "career.orders.title",
          "career.orders.summary",
          `
            <div class="table-shell">
              <div class="table-row table-head">
                <span>${await t("career.orders.origin")}</span>
                <span>${await t("career.orders.destination")}</span>
                <span>${await t("career.orders.eta")}</span>
                <span>${await t("career.orders.payout")}</span>
              </div>
              <div class="table-row"><span>Berlin</span><span>Prague</span><span>05h 40m</span><span>EUR 18,200</span></div>
              <div class="table-row"><span>Hamburg</span><span>Lyon</span><span>12h 15m</span><span>EUR 31,980</span></div>
              <div class="table-row"><span>Warsaw</span><span>Vienna</span><span>09h 05m</span><span>EUR 22,460</span></div>
            </div>
          `
        );
      case "freight":
        return buildSectionFrame(
          "career.nav.freight",
          "career.freight.title",
          "career.freight.summary",
          `
            <div class="filters-row">
              <span class="filter-chip">${await t("career.freight.filters_distance")}</span>
              <span class="filter-chip">${await t("career.freight.filters_profit")}</span>
              <span class="filter-chip">${await t("career.freight.filters_risk")}</span>
              <span class="filter-chip">${await t("career.freight.filters_time")}</span>
            </div>
            <div class="detail-grid three-up">
              <div class="detail-card" style="grid-column: span 2;">
                <div class="table-shell">
                  <div class="table-row table-head">
                    <span>${await t("career.freight.route")}</span>
                    <span>${await t("career.freight.revenue")}</span>
                    <span>${await t("career.freight.cost_breakdown")}</span>
                    <span>${await t("career.freight.accept")}</span>
                  </div>
                  <div class="table-row"><span>Rotterdam - Basel</span><span>EUR 24,600</span><span>Fuel / Toll / Insurance</span><button class="table-action">${await t("career.freight.accept")}</button></div>
                  <div class="table-row"><span>Oslo - Malmo</span><span>EUR 11,980</span><span>Fuel / Ferry / Repair</span><button class="table-action">${await t("career.freight.accept")}</button></div>
                </div>
              </div>
              <div class="map-preview"><div class="map-grid"><span></span></div></div>
            </div>
          `
        );
      case "dispatcher":
        return buildSectionFrame(
          "career.nav.dispatcher",
          "career.dispatcher.title",
          "career.dispatcher.summary",
          buildDetailCards([
            { label: await t("career.dispatcher.priority_high"), value: "Medical route / Berlin", copy: await t("career.dispatcher.reason_level") },
            { label: await t("career.dispatcher.priority_medium"), value: "Retail freight / Hamburg", copy: await t("career.dispatcher.reason_location") },
            { label: await t("career.dispatcher.priority_high"), value: "Industrial steel / Prague", copy: await t("career.dispatcher.reason_demand") },
          ])
        );
      case "livemap":
        return buildSectionFrame(
          "career.nav.livemap",
          "career.livemap.title",
          "career.livemap.summary",
          `
            <div class="detail-grid three-up">
              <div class="map-preview" style="grid-column: span 2;"><div class="map-grid"><span></span></div></div>
              <div class="detail-grid">
                <article class="detail-card">
                  <span>${await t("career.livemap.tracking")}</span>
                  <strong>North Axis 014</strong>
                  <p>${await t("career.livemap.route_signal")}</p>
                </article>
                <article class="detail-card">
                  <span>${await t("career.livemap.convoy_status")}</span>
                  <strong>On route</strong>
                  <p>Fuel and rest windows are inside target thresholds.</p>
                </article>
              </div>
            </div>
          `
        );
      case "fleet":
        return buildSectionFrame(
          "career.nav.fleet",
          "career.fleet.title",
          "career.fleet.summary",
          `
            <div class="table-shell">
              <div class="table-row table-head">
                <span>${await t("career.fleet.truck")}</span>
                <span>${await t("career.fleet.condition")}</span>
                <span>${await t("career.fleet.assigned_driver")}</span>
                <span>${await t("career.fleet.maintenance")}</span>
              </div>
              <div class="table-row"><span>${window.playerTruck?.name || "Primary Truck"}</span><span>92%</span><span>Player</span><span>Routine check in 1,250 km</span></div>
              <div class="table-row"><span>Fleet Summary</span><span>${truckCount} / ${trailerCount}</span><span>${companyName}</span><span>Monitoring active</span></div>
            </div>
          `
        );
      case "balance":
        return buildSectionFrame(
          "career.nav.balance",
          "career.balance.title",
          "career.balance.summary",
          `
            ${buildDetailCards([
              { label: await t("career.balance.cash_balance"), value: formatCurrency(money), copy: companyName },
              { label: await t("career.balance.fuel"), value: "EUR 5,820", copy: "Rolling fuel exposure" },
              { label: await t("career.balance.repairs"), value: "EUR 1,740", copy: "Current workshop demand" },
              { label: await t("career.balance.salaries"), value: "EUR 12,480", copy: "Driver payroll stack" },
            ], "four-up")}
            <div class="finance-chart"><span></span></div>
          `
        );
      case "insurance":
        return buildSectionFrame(
          "career.nav.insurance",
          "career.insurance.title",
          "career.insurance.summary",
          `
            <div class="detail-grid three-up">
              <article class="plan-card"><span>${await t("career.insurance.basic")}</span><strong>EUR 280 / month</strong><p>${await t("career.insurance.feature_damage")}</p></article>
              <article class="plan-card featured"><span>${await t("career.insurance.basic_pro")}</span><strong>EUR 520 / month</strong><p>${await t("career.insurance.feature_repair")}</p></article>
              <article class="plan-card"><span>${await t("career.insurance.supporter")}</span><strong>Invite only</strong><p>${await t("career.insurance.feature_bonus")}</p></article>
            </div>
          `
        );
      case "statistics":
        return buildSectionFrame(
          "career.nav.statistics",
          "career.statistics.title",
          "career.statistics.summary",
          buildDetailCards([
            { label: await t("career.statistics.profit"), value: "EUR 284,000" },
            { label: await t("career.statistics.kilometers"), value: "48,920 km" },
            { label: await t("career.statistics.efficiency"), value: "92%" },
            { label: await t("career.statistics.utilization"), value: "81%" },
          ], "four-up")
        );
      case "achievements":
        return buildSectionFrame(
          "career.nav.achievements",
          "career.achievements.title",
          "career.achievements.summary",
          buildDetailCards([
            { label: await t("career.achievements.level"), value: String(level) },
            { label: await t("career.achievements.xp"), value: formatTelemetryNumber(xp, 0) },
            { label: await t("career.achievements.reputation"), value: `L${level}` },
          ])
        );
      case "company":
        return buildSectionFrame(
          "career.nav.company",
          "career.company.title",
          "career.company.summary",
          buildDetailCards([
            { label: await t("career.company.headquarters"), value: companyName },
            { label: await t("career.company.staff_capacity"), value: "24 members" },
            { label: await t("career.company.growth"), value: "Central Europe expansion" },
          ])
        );
      case "store":
        return buildSectionFrame(
          "career.nav.store",
          "career.store.title",
          "career.store.summary",
          buildDetailCards([
            { label: await t("career.store.phone"), value: "Dispatch sync", copy: await t("career.store.efficiency_bonus") },
            { label: await t("career.store.care"), value: "Cabin care pack", copy: await t("career.store.comfort_bonus") },
            { label: await t("career.store.food"), value: "Meal stock", copy: await t("career.store.fatigue_bonus") },
            { label: await t("career.store.accessories"), value: "Driver setup", copy: await t("career.store.efficiency_bonus") },
          ], "four-up")
        );
      case "plugin":
        return buildSectionFrame(
          "career.nav.plugin",
          "career.plugin.title",
          "career.plugin.summary",
          `
            <div class="detail-grid three-up">
              <article class="detail-card">
                <span>${await t("career.plugin.auto_detect")}</span>
                <strong>ETS2 / ATS</strong>
                <p>${await t("career.plugin.install_help")}</p>
              </article>
              <article class="detail-card">
                <span>${await t("career.plugin.status_label")}</span>
                <strong id="careerPluginStatusValue">${uiText.pluginOffline}</strong>
                <p id="careerPluginDetailValue">${uiText.pluginMissingDetail}</p>
              </article>
              <article class="detail-card">
                <span>${await t("career.status.sdk_connected")}</span>
                <strong id="careerBridgeStatusValue">${uiText.bridgeOffline}</strong>
                <p id="careerBridgeDetailValue">${uiText.bridgeOfflineDetail}</p>
              </article>
            </div>
          `
        );
      case "settings":
      default:
        return buildSectionFrame(
          "career.nav.settings",
          "career.settings.title",
          "career.settings.summary",
          buildDetailCards([
            { label: await t("career.settings.theme"), value: localStorage.getItem("theme") || "neon" },
            { label: await t("career.settings.language"), value: "System controlled" },
            { label: await t("career.settings.modules"), value: "Career / Utility / Telemetry" },
          ])
        );
    }
  };

  const setLamp = (element, active) => element?.classList.toggle("is-active", Boolean(active));
  const setCareerGame = (game) => {
    const label = (game || "ets2").toUpperCase();
    if (refs.careerHeroTitle) refs.careerHeroTitle.textContent = label;
    if (refs.careerGameLabel) refs.careerGameLabel.textContent = label;
  };
  let activeCareerPanel = "dashboard";

  const setHubVisibility = (visible) => {
    refs.hubScreen?.classList.toggle("is-hidden", !visible);
  };

  const applyCareerState = () => {
    setLamp(refs.statusGameRunning, careerState.gameRunning);
    setLamp(refs.statusPluginInstalled, careerState.pluginInstalled);
    setLamp(refs.statusSdkConnected, careerState.bridgeConnected);
    setCareerGame(careerState.activeGame || lastSelectedGame || "ets2");

    const pluginStatusValue = document.getElementById("careerPluginStatusValue");
    const pluginDetailValue = document.getElementById("careerPluginDetailValue");
    const bridgeStatusValue = document.getElementById("careerBridgeStatusValue");
    const bridgeDetailValue = document.getElementById("careerBridgeDetailValue");

    if (pluginStatusValue) pluginStatusValue.textContent = careerState.pluginInstalled ? uiText.pluginOnline : uiText.pluginOffline;
    if (pluginDetailValue) pluginDetailValue.textContent = careerState.pluginInstalled ? uiText.pluginInstalledDetail : uiText.pluginMissingDetail;
    if (bridgeStatusValue) bridgeStatusValue.textContent = careerState.bridgeConnected ? uiText.bridgeOnline : uiText.bridgeOffline;
    if (bridgeDetailValue) bridgeDetailValue.textContent = careerState.bridgeConnected ? uiText.bridgeOnlineDetail : uiText.bridgeOfflineDetail;

    if (!refs.careerConnectionNote) return;
    if (careerState.bridgeConnected && careerState.paused) {
      refs.careerConnectionNote.textContent = careerText.paused;
      return;
    }
    if (careerState.bridgeConnected) {
      refs.careerConnectionNote.textContent = careerText.live;
      return;
    }
    if (!careerState.pluginInstalled) {
      refs.careerConnectionNote.textContent = careerText.missingPlugin;
      return;
    }
    if (!careerState.gameRunning) {
      refs.careerConnectionNote.textContent = careerText.gameStopped;
      return;
    }
    refs.careerConnectionNote.textContent = careerText.waiting;
  };

  const applyHubMode = (mode) => {
    const isCareer = mode === "career";
    document.body.classList.toggle("mode-career", isCareer);
    document.body.classList.toggle("mode-editor", !isCareer);
    refs.editorModeBtn?.classList.toggle("active", !isCareer);
    refs.careerModeBtn?.classList.toggle("active", isCareer);
  };

  const updateEditorStage = async (tab) => {
    const meta = editorStageMeta[tab] || editorStageMeta.profile;
    if (refs.editorStageTitle) refs.editorStageTitle.textContent = await t(meta.title);
    if (refs.editorStageSummary) refs.editorStageSummary.textContent = await t(meta.summary);
  };

  const updateOperationalOverview = () => {
    const money = Number(window.currentProfileData?.money ?? 0);
    const xp = Number(window.currentProfileData?.xp ?? 0);
    const level = deriveLevel(xp);
    const truckCount = window.allTrucks?.length || 0;
    const trailerCount = window.allTrailers?.length || 0;
    const profileLabel = window.selectedProfilePath
      ? refs.profileNameDisplay?.textContent?.trim() || uiText.noProfile
      : uiText.noProfile;
    const saveLabel = window.selectedSavePath
      ? refs.saveNameDisplay?.textContent?.trim() || uiText.noSave
      : uiText.noSave;
    const companyLabel = window.selectedProfilePath ? profileLabel : uiText.noCompany;

    if (refs.editorProfileValue) refs.editorProfileValue.textContent = profileLabel;
    if (refs.editorSaveValue) refs.editorSaveValue.textContent = saveLabel;
    if (refs.editorMoneyValue) refs.editorMoneyValue.textContent = formatCurrency(money);
    if (refs.editorXpValue) refs.editorXpValue.textContent = formatTelemetryNumber(xp, 0);
    if (refs.editorLevelValue) refs.editorLevelValue.textContent = String(level);
    if (refs.editorFleetValue) refs.editorFleetValue.textContent = `${truckCount} / ${trailerCount}`;

    if (refs.careerCompanyValue) refs.careerCompanyValue.textContent = companyLabel;
    if (refs.careerSidebarCompany) refs.careerSidebarCompany.textContent = companyLabel;
    if (refs.careerSidebarBalance) refs.careerSidebarBalance.textContent = formatCurrency(money);
    if (refs.careerBalanceValue) refs.careerBalanceValue.textContent = formatCurrency(money);
    if (refs.careerReputationValue) refs.careerReputationValue.textContent = `L${level}`;
    if (refs.careerFleetStatusValue) refs.careerFleetStatusValue.textContent = `${truckCount} / ${trailerCount}`;
    if (refs.careerLiveRevenueValue) refs.careerLiveRevenueValue.textContent = formatCurrency(Math.max(12480, level * 3200));
    if (refs.careerCostFuelValue) refs.careerCostFuelValue.textContent = formatCurrency(80 + truckCount * 48);
    if (refs.careerCostRepairValue) refs.careerCostRepairValue.textContent = formatCurrency(24 + trailerCount * 50);
    if (refs.careerCostTollValue) refs.careerCostTollValue.textContent = formatCurrency(18 + level * 6);
    if (refs.careerDriversOnlineValue) refs.careerDriversOnlineValue.textContent = String(Math.max(1, Math.min(level, 9))).padStart(2, "0");
    if (refs.careerDriversRestingValue) refs.careerDriversRestingValue.textContent = String(Math.max(1, Math.min(Math.ceil(level / 3), 4))).padStart(2, "0");
  };

  const showCareerPanel = async (panel) => {
    activeCareerPanel = panel;
    careerNavButtons.forEach((button) => button.classList.toggle("active", button.dataset.careerPanel === panel));

    const isDashboard = panel === "dashboard";
    refs.careerDashboardShell?.classList.toggle("is-hidden", !isDashboard);
    refs.careerDetailHost?.classList.toggle("is-hidden", isDashboard);

    if (isDashboard) {
      if (refs.careerDetailHost) refs.careerDetailHost.innerHTML = "";
      return;
    }

    if (refs.careerDetailHost) {
      refs.careerDetailHost.innerHTML = await renderCareerDetailPanel(panel);
    }
    applyCareerState();
  };

  const refreshCareerPanel = async () => {
    updateOperationalOverview();
    await showCareerPanel(activeCareerPanel);
  };

  const updateUIWithCurrentQuicksave = () => {
    void refreshCareerPanel();
  };

  const setEditorPresentationMode = (mode) => {
    const isAdvanced = mode === "advanced";
    document.body.classList.toggle("editor-advanced", isAdvanced);
    document.body.classList.toggle("editor-safe", !isAdvanced);
    refs.saveSafeModeBtn?.classList.toggle("active", !isAdvanced);
    refs.saveAdvancedModeBtn?.classList.toggle("active", isAdvanced);
    if (refs.editorModeNotice) {
      refs.editorModeNotice.textContent = isAdvanced ? uiText.advancedMode : uiText.safeMode;
    }
    localStorage.setItem("ets2_editor_mode", isAdvanced ? "advanced" : "safe");
  };

  const renderTelemetry = (data) => {
    const speed = Number(data?.speed_kph ?? 0);
    const fuel = Number(data?.fuel_liters ?? 0);
    const capacity = Number(data?.fuel_capacity_liters ?? 0);
    const ratio = capacity > 0 ? Math.max(0, Math.min(fuel / capacity, 1)) : 0;
    careerState.bridgeConnected = true;
    careerState.paused = Number(data?.paused ?? 0) === 1;
    if (refs.careerSpeedDial) refs.careerSpeedDial.style.setProperty("--dial-progress", String(Math.min(speed / 180, 1)));
    if (refs.careerSpeedValue) refs.careerSpeedValue.textContent = String(Math.round(speed));
    if (refs.careerGearValue) {
      const gear = Number(data?.gear ?? 0);
      refs.careerGearValue.textContent = gear === 0 ? "N" : gear > 0 ? String(gear) : `R${Math.abs(gear)}`;
    }
    if (refs.careerFuelValue) {
      refs.careerFuelValue.textContent =
        capacity > 0
          ? `${formatTelemetryNumber(fuel, 1)} / ${formatTelemetryNumber(capacity, 1)} L`
          : `${formatTelemetryNumber(fuel, 1)} L`;
    }
    if (refs.careerFuelPercent) refs.careerFuelPercent.textContent = `${Math.round(ratio * 100)}%`;
    if (refs.careerFuelBarFill) refs.careerFuelBarFill.style.setProperty("--fuel-progress", String(ratio));
    if (refs.careerRpmValue) refs.careerRpmValue.textContent = formatTelemetryNumber(data?.engine_rpm ?? 0, 0);
    applyCareerState();
  };

  const renderCareerStatus = (status) => {
    careerState.gameRunning = Boolean(status?.ets2_running || status?.ats_running);
    careerState.pluginInstalled = Boolean(status?.plugin_installed);
    careerState.bridgeConnected = Boolean(status?.bridge_connected);
    careerState.activeGame = status?.active_game || careerState.activeGame || lastSelectedGame || "ets2";
    if (!careerState.bridgeConnected) {
      careerState.paused = false;
    }
    applyCareerState();
  };
  const updateCareerFlag = (key, value) => {
    careerState[key] = Boolean(value);
    if (key === "bridgeConnected" && !careerState.bridgeConnected) {
      careerState.paused = false;
    }
    applyCareerState();
  };

  const activateMode = async (mode) => {
    applyHubMode(mode);
    setHubVisibility(false);
    try {
      await invoke("hub_set_mode", { mode });
    } catch (error) {
      console.error(error);
    }
  };

  document.addEventListener("editor-tab-changed", (event) => {
    void updateEditorStage(event.detail.tab);
  });

  careerNavButtons.forEach((button) => {
    button.addEventListener("click", () => {
      void showCareerPanel(button.dataset.careerPanel);
    });
  });

  refs.editorModeBtn?.addEventListener("click", () => activateMode("utility"));
  refs.careerModeBtn?.addEventListener("click", () => activateMode("career"));
  refs.hubEditorCard?.addEventListener("click", () => activateMode("utility"));
  refs.hubCareerCard?.addEventListener("click", () => activateMode("career"));
  refs.hubHomeBtn?.addEventListener("click", () => setHubVisibility(true));
  refs.saveSafeModeBtn?.addEventListener("click", () => setEditorPresentationMode("safe"));
  refs.saveAdvancedModeBtn?.addEventListener("click", () => setEditorPresentationMode("advanced"));

  listen("hub://mode_changed", (event) => applyHubMode(event.payload.mode ?? event.payload)).catch(console.error);
  listen("career://game_running", (event) => updateCareerFlag("gameRunning", event.payload)).catch(console.error);
  listen("career://plugin_installed", (event) => updateCareerFlag("pluginInstalled", event.payload)).catch(console.error);
  listen("career://bridge_connected", (event) => updateCareerFlag("bridgeConnected", event.payload)).catch(console.error);
  listen("career://status", (event) => renderCareerStatus(event.payload)).catch(console.error);
  listen("career://telemetry_tick", (event) => renderTelemetry(event.payload)).catch(console.error);

  try {
    applyHubMode(await invoke("hub_get_mode"));
  } catch {
    applyHubMode("utility");
  }

  try {
    const selectedGame = await invoke("get_selected_game");
    careerState.activeGame = selectedGame;
    setCareerGame(selectedGame);
    lastSelectedGame = selectedGame;
  } catch {}

  try {
    renderCareerStatus(await invoke("career_get_status"));
  } catch {}

  try {
    updateCareerFlag("pluginInstalled", await invoke("get_plugin_status"));
  } catch {}

  setEditorPresentationMode(localStorage.getItem("ets2_editor_mode") || "safe");
  await updateEditorStage(activeTab);
  await showCareerPanel("dashboard");
  updateOperationalOverview();

  if (refs.versionBtn) {
    refs.versionBtn.textContent = `v${await appVersion()}`;
    refs.versionBtn.addEventListener("click", () => manualUpdateCheck(window.showToast));
  }

  setTimeout(() => checkUpdaterOnStartup(window.showToast), 2000);

  refs.websiteBtn?.addEventListener("click", () => openUrl("https://www.xlieferant.dev/"));
  refs.youtubeBtn?.addEventListener("click", () => openUrl("https://www.youtube.com/@xLieferant"));
  refs.patreonBtn?.addEventListener("click", () => openUrl("https://www.patreon.com/cw/xLieferant"));
  refs.githubBtn?.addEventListener("click", () => openUrl("https://github.com/xLieferant/Save-Edit-Tool"));

  const switchGame = async (game) => {
    try {
      await invoke("set_selected_game", { game });
      localStorage.setItem("ets2_force_profile_picker_open", "1");
      location.reload();
    } catch (error) {
      showToast("toasts.generic_error_prefix", { error: error.toString() }, "error");
    }
  };

  refs.ets2Btn?.addEventListener("click", () => switchGame("ets2"));
  refs.atsBtn?.addEventListener("click", () => switchGame("ats"));

  window.selectedProfilePath = null;
  window.selectedSavePath = null;
  window.currentSavePath = null;
  window.currentProfileData = {};
  window.currentQuicksaveData = {};
  window.readSaveGameConfig = {};
  window.baseConfig = {};
  window.allTrucks = [];
  window.playerTruck = null;
  window.allTrailers = [];
  window.playerTrailer = null;
  window.extractPlateText = (plate) => (plate ? plate.replace(/^"|"$/g, "") : "");

  const closeDropdowns = () => {
    refs.profileDropdownList.classList.remove("show");
    refs.saveDropdownList.classList.remove("show");
  };

  document.addEventListener("click", (event) => {
    if (!event.target.closest(".profile-picker")) refs.profileDropdownList.classList.remove("show");
    if (!event.target.closest(".save-picker")) refs.saveDropdownList.classList.remove("show");
  });

  document.querySelector(".profile-picker")?.addEventListener("click", (event) => {
    if (event.target.closest(".custom-dropdown-list")) return;
    event.stopPropagation();
    const open = refs.profileDropdownList.classList.contains("show");
    closeDropdowns();
    if (!open) refs.profileDropdownList.classList.add("show");
  });

  document.querySelector(".save-picker")?.addEventListener("click", (event) => {
    if (event.target.closest(".dropdown-list") || !window.selectedProfilePath) return;
    event.stopPropagation();
    const open = refs.saveDropdownList.classList.contains("show");
    closeDropdowns();
    if (!open) refs.saveDropdownList.classList.add("show");
  });

  if (localStorage.getItem("ets2_force_profile_picker_open") === "1") {
    localStorage.removeItem("ets2_force_profile_picker_open");
    refs.profileDropdownList.classList.add("show");
  }

  const loadProfileData = async () => {
    window.currentProfileData = await invoke("read_all_save_data");
  };
  const loadQuicksave = async () => {
    window.currentQuicksaveData = await invoke("quicksave_game_info");
  };
  const loadProfileSaveConfig = async () => {
    window.readSaveGameConfig = await invoke("read_save_config", { profilePath: window.selectedProfilePath });
  };
  const loadBaseConfig = async () => {
    window.baseConfig = await invoke("read_base_config");
  };
  const loadAllTrucks = async () => {
    try {
      window.playerTruck = await invoke("get_player_truck", { profilePath: window.selectedProfilePath });
      window.allTrucks = [window.playerTruck];
    } catch {
      window.playerTruck = null;
      window.allTrucks = [];
    }
  };
  const loadAllTrailers = async () => {
    try {
      const trailer = await invoke("get_player_trailer", { profilePath: window.selectedProfilePath });
      window.playerTrailer = trailer || null;
      window.allTrailers = trailer ? [trailer] : [];
    } catch {
      window.playerTrailer = null;
      window.allTrailers = [];
    }
  };

  window.loadProfileData = loadProfileData;
  window.loadQuicksave = loadQuicksave;
  window.loadProfileSaveConfig = loadProfileSaveConfig;
  window.loadBaseConfig = loadBaseConfig;
  window.loadAllTrucks = loadAllTrucks;
  window.loadAllTrailers = loadAllTrailers;

  const syncSelectedGameUi = async () => {
    try {
      const game = await invoke("get_selected_game");
      const previousGame = lastSelectedGame;
      lastSelectedGame = game;
      setCareerGame(game);
      setLamp(refs.statusPluginInstalled, await invoke("get_plugin_status"));
      refs.ets2Btn.classList.toggle("active", game !== "ats");
      refs.ets2Btn.disabled = game !== "ats";
      refs.atsBtn.classList.toggle("active", game === "ats");
      refs.atsBtn.disabled = game === "ats";
      if (game !== previousGame) {
        updateToolImagesForGame(game);
        loadTools(activeTab);
      } else {
        updateToolImagesForGame(game);
      }
    } catch (error) {
      console.warn("Game sync failed:", error);
    }
  };

  const loadSelectedSave = async () => {
    window.logUserAction("load_save", "start");
    try {
      refs.profileStatus.textContent = uiText.loadingSave;
      await loadProfileData();
      await loadQuicksave();
      await loadProfileSaveConfig();
      await loadBaseConfig();
      await loadAllTrucks();
      await loadAllTrailers();
      updateOperationalOverview();
      await showCareerPanel(activeCareerPanel);
      refs.profileStatus.textContent = uiText.saveLoaded;
      window.showToast("toasts.save_loaded_success", {}, "success");
      loadTools(activeTab);
      window.logUserAction("load_save", "success");
    } catch (error) {
      console.error(error);
      window.showToast("toasts.save_load_error", {}, "error");
      window.logUserAction("load_save", "error");
    }
  };

  const scanSavesForProfile = async () => {
    if (!window.selectedProfilePath) return;
    refs.saveDropdownList.innerHTML = "";
    refs.openSaveModalBtn.disabled = false;
    try {
      const saves = (await invoke("find_profile_saves", { profilePath: window.selectedProfilePath }))
        .filter((save) => save.success && save.kind !== "Invalid")
        .sort((a, b) => {
          const priority = (folder) => folder === "quicksave" ? 0 : folder === "autosave" ? 1 : 2;
          const pA = priority(a.folder.toLowerCase());
          const pB = priority(b.folder.toLowerCase());
          return pA !== pB ? pA - pB : b.folder.localeCompare(a.folder, undefined, { numeric: true });
        });
      saves.forEach((save) => {
        const item = document.createElement("div");
        item.className = "dropdown-item";
        item.textContent =
          save.folder.toLowerCase() === "quicksave"
            ? "~ Quicksave ~"
            : save.folder.toLowerCase() === "autosave"
              ? "~ Autosave ~"
              : `${save.name ?? save.folder} [${save.folder}]`;
        item.addEventListener("click", async () => {
          window.selectedSavePath = save.path;
          window.currentSavePath = save.path;
          refs.saveNameDisplay.textContent = save.name ?? save.folder;
          refs.saveDropdownList.classList.remove("show");
          await invoke("load_profile", { profilePath: window.selectedProfilePath, savePath: save.path });
          await loadSelectedSave();
        });
        refs.saveDropdownList.appendChild(item);
      });
    } catch (error) {
      console.error(error);
      showToast("toasts.scan_saves_error", {}, "error");
    }
  };

  const loadSelectedProfile = async () => {
    if (!window.selectedProfilePath) return;
    window.logUserAction("load_profile", "start");
    try {
      refs.profileStatus.textContent = uiText.loadingProfile;
      refs.saveDropdownList.innerHTML = "";
      window.selectedSavePath = null;
      window.currentSavePath = null;
      refs.saveNameDisplay.textContent = uiText.noSave;
      await invoke("set_active_profile", { profilePath: window.selectedProfilePath });
      await scanSavesForProfile();
      try {
        await loadBaseConfig();
        await loadProfileSaveConfig();
      } catch {}
      window.currentProfileData = {};
      window.currentQuicksaveData = {};
      window.allTrucks = [];
      window.playerTruck = null;
      window.allTrailers = [];
      window.playerTrailer = null;
      const cached = JSON.parse(localStorage.getItem("ets2_profiles_cache") || "[]");
      const profileInfo = cached.find((profile) => profile.path === window.selectedProfilePath);
      window.selectedProfileHasAvatar = !!profileInfo?.avatar;
      const iconSrc = resolveProfileIcon(profileInfo);
      const footerIcon = document.getElementById("activeProfileIcon");
      if (footerIcon) {
        footerIcon.src = iconSrc;
        if (window.selectedProfileHasAvatar) footerIcon.onerror = () => handleIconError(footerIcon);
      }
      updateOperationalOverview();
      await showCareerPanel(activeCareerPanel);
      refs.profileStatus.textContent = uiText.profileLoaded;
      window.showToast("toasts.profile_loaded_select_save", {}, "info");
      loadTools(activeTab);
      window.logUserAction("load_profile", "success");
    } catch (error) {
      console.error(error);
      refs.profileStatus.textContent = uiText.profileError;
      window.showToast("toasts.profile_load_error", {}, "error");
      window.logUserAction("load_profile", "error");
    }
  };

  const createProfileItem = (profile) => {
    const item = document.createElement("div");
    item.className = "dropdown-item";
    const img = document.createElement("img");
    img.src = resolveProfileIcon(profile);
    img.className = "profile-icon-dropdown";
    if (profile.avatar) {
      img.dataset.hasAvatar = "true";
      img.onerror = () => handleIconError(img);
    }
    const label = document.createElement("span");
    label.textContent = profile.name;
    item.appendChild(img);
    item.appendChild(label);
    item.addEventListener("click", async () => {
      window.selectedProfilePath = profile.path;
      refs.profileNameDisplay.textContent = profile.name;
      refs.profileDropdownList.classList.remove("show");
      localStorage.setItem("ets2_last_profile", profile.path);
      try {
        await invoke("save_last_profile", { profilePath: profile.path });
      } catch {}
      await loadSelectedProfile();
    });
    return item;
  };

  const scanProfiles = async ({ saveToBackend = true, showToasts = true } = {}) => {
    refs.profileStatus.textContent = uiText.scanningProfiles;
    refs.profileDropdownList.innerHTML = "";
    window.logUserAction("scan_profiles", "start");
    try {
      await syncSelectedGameUi();
      const profiles = await invoke("find_ets2_profiles");
      refs.profileStatus.textContent = await t("status_text.profiles_found", { count: profiles.length });
      if (showToasts) window.showToast("toasts.profiles_found", {}, "success");
      profiles.filter((profile) => profile.success).forEach((profile) => {
        refs.profileDropdownList.appendChild(createProfileItem(profile));
      });
      localStorage.setItem("ets2_profiles_cache", JSON.stringify(profiles));
      if (saveToBackend) {
        await invoke("save_profiles_cache", {
          profiles: profiles.map((profile) => ({
            path: profile.path,
            name: profile.name ?? null,
            avatar: profile.avatar ?? null,
            success: !!profile.success,
            message: profile.message ?? null,
          })),
        });
      }
      const remoteLast = await invoke("read_last_profile").catch(() => null);
      const last = remoteLast || localStorage.getItem("ets2_last_profile");
      if (last) {
        const matched = profiles.find((profile) => profile.path === last && profile.success);
        if (matched) {
          window.selectedProfilePath = matched.path;
          refs.profileNameDisplay.textContent = matched.name ?? "Unknown";
          await loadSelectedProfile();
          return;
        }
      }
      updateOperationalOverview();
      window.logUserAction("scan_profiles", "success");
    } catch (error) {
      console.error(error);
      refs.profileStatus.textContent = uiText.scanFailed;
      window.showToast("toasts.no_profiles_found", {}, "error");
      window.logUserAction("scan_profiles", "error");
    }
  };

  refs.refreshBtn?.addEventListener("click", () => scanProfiles({ saveToBackend: true, showToasts: true }));

  refs.cloneBtn?.addEventListener("click", async () => {
    if (!window.selectedProfilePath) {
      showToast("toasts.no_profile_selected", {}, "warning");
      return;
    }
    const choice = await openModalMulti("Manage Profile", [{
      type: "dropdown",
      id: "action",
      label: "Action",
      value: "Duplicate",
      options: ["Duplicate", "Rename"],
    }]);
    if (!choice) return;
    if (choice.action === "Duplicate") return openCloneProfileModal();
    const currentName = refs.profileNameDisplay.textContent;
    const newName = await openModalText("Rename Profile", "New Name", currentName);
    if (newName && newName.trim() !== "" && newName !== currentName) {
      try {
        window.selectedProfilePath = await invoke("profile_rename", { newName: newName.trim() });
        refs.profileNameDisplay.textContent = newName.trim();
        showToast("toasts.profile_renamed_success", {}, "success");
        await scanProfiles({ saveToBackend: true, showToasts: false });
        await loadSelectedProfile();
      } catch (error) {
        showToast("toasts.profile_rename_error", { error: error.toString() }, "error");
      }
    }
  });

  window.handleCopyControls = async function () {
    if (!window.selectedProfilePath) {
      showToast("toasts.no_source_profile_selected", {}, "warning");
      return;
    }
    const profiles = await invoke("find_ets2_profiles");
    const targets = profiles.filter((profile) => profile.success && profile.path !== window.selectedProfilePath);
    if (!targets.length) {
      showToast("toasts.no_other_profiles", {}, "warning");
      return;
    }
    const result = await openModalMulti("Copy Controls", [{
      type: "dropdown",
      id: "target",
      label: "Target Profile",
      value: `${targets[0].name} [${targets[0].path}]`,
      options: targets.map((profile) => `${profile.name} [${profile.path}]`),
    }]);
    if (!result?.target) return;
    const target = targets.find((profile) => `${profile.name} [${profile.path}]` === result.target);
    if (!target) return;
    await invoke("copy_profile_controls", {
      sourceProfilePath: window.selectedProfilePath,
      targetProfilePath: target.path,
    });
    showToast("toasts.copy_controls_success", {}, "success");
  };

  window.handleMoveMods = async function () {
    if (!window.selectedProfilePath) {
      showToast("toasts.no_source_profile_selected", {}, "warning");
      return;
    }
    const profiles = await invoke("find_ets2_profiles");
    const targets = profiles.filter((profile) => profile.success && profile.path !== window.selectedProfilePath);
    if (!targets.length) {
      showToast("toasts.no_other_valid_profiles", {}, "warning");
      return;
    }
    const result = await openModalMulti("Move Mods", [{
      type: "dropdown",
      id: "target",
      label: "Target Profile",
      value: `${targets[0].name} [${targets[0].path}]`,
      options: targets.map((profile) => `${profile.name} [${profile.path}]`),
    }]);
    if (!result?.target) return;
    const target = targets.find((profile) => `${profile.name} [${profile.path}]` === result.target);
    if (!target) return;
    const message = await invoke("copy_mods_to_profile", { targetProfilePath: target.path });
    const count = message.match(/\d+/)?.[0] ?? "?";
    showToast("toasts.move_mods_success", { count }, "success");
  };

  window.showLanguagePicker = async function () {
    try {
      const languages = await invoke("get_available_languages_command");
      const currentLanguage = await invoke("get_current_language_command");

      if (!languages?.length) {
        showToast("toasts.load_languages_error", { error: "No languages available" }, "error");
        return;
      }

      const optionLabels = languages.reduce((acc, language) => {
        acc[language.code] = language.name;
        return acc;
      }, {});

      const result = await openModalMulti("tools.settings.language.modalTextTitle", [
        {
          type: "dropdown",
          id: "language",
          label: "label.label_language",
          value: currentLanguage,
          options: languages.map((language) => language.code),
          optionLabels,
        },
      ]);

      if (!result?.language || result.language === currentLanguage) {
        return;
      }

      await invoke("set_language_command", { language: result.language });
      showToast("toasts.language_updated", {}, "success");
      location.reload();
    } catch (error) {
      console.error("Language picker failed:", error);
      showToast("toasts.load_languages_error", { error: error.toString() }, "error");
    }
  };

  setInterval(async () => {
    if (!window.selectedSavePath) return;
    try {
      await loadQuicksave();
      updateUIWithCurrentQuicksave();
    } catch {}
  }, 300000);

  try {
    const cached = await invoke("read_profiles_cache");
    if (cached?.length) {
      refs.profileDropdownList.innerHTML = "";
      cached.filter((profile) => profile.success).forEach((profile) => {
        refs.profileDropdownList.appendChild(createProfileItem(profile));
      });
    }
  } catch {}

  await scanProfiles({ saveToBackend: true, showToasts: true });
  window.dispatchEvent(new Event("translations-ready"));
});
