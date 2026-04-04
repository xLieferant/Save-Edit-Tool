import { loadTools, activeTab, openCloneProfileModal, openModalMulti, openModalText } from "./app.js";
import { updateToolImagesForGame } from "./tools.js";

// Allow loading the UI in a normal browser (no Tauri APIs) so the HTML preview
// still has working click handlers and basic navigation.
const tauri = window.__TAURI__;
const hasTauri = Boolean(tauri?.core?.invoke);

const app = hasTauri ? tauri.app : null;
const openUrl = hasTauri ? tauri.opener.openUrl : async () => {};
const tauriInvoke = hasTauri ? tauri.core.invoke : async () => {
  throw new Error("Tauri API not available");
};
const convertFileSrc = hasTauri ? tauri.core.convertFileSrc : (path) => path;
const listen = hasTauri ? tauri.event.listen : async () => () => {};

let applySetting = () => {};
let checkUpdaterOnStartup = () => {};
let manualUpdateCheck = () => {};

let lastSelectedGame = null;
const CAREER_LOAD_ERROR = "Career Mode failed to load";
const isStandaloneEditorPage = Boolean(window.__ETS2_STANDALONE_EDITOR__);

function ensureUiErrorBanner() {
  let banner = document.getElementById("careerLoadFallback");
  if (!banner && document.body) {
    banner = document.createElement("div");
    banner.id = "careerLoadFallback";
    banner.style.cssText = [
      "position: fixed",
      "top: 16px",
      "right: 16px",
      "z-index: 5000",
      "padding: 12px 16px",
      "border-radius: 14px",
      "background: rgba(255, 77, 79, 0.94)",
      "color: #ffffff",
      "font: 600 14px/1.4 Bahnschrift, Segoe UI, sans-serif",
      "box-shadow: 0 18px 42px rgba(0, 0, 0, 0.28)",
      "display: none",
    ].join("; ");
    document.body.appendChild(banner);
  }
  return banner;
}

function showCareerLoadFailure(error) {
  console.error("[career] fallback activated", error);
  const banner = ensureUiErrorBanner();
  if (banner) {
    banner.textContent = CAREER_LOAD_ERROR;
    banner.style.display = "block";
  }
  document.body?.classList.remove("mode-career");
  document.body?.classList.add("mode-editor");
  document.getElementById("hubScreen")?.classList.add("is-hidden");
  const profileStatus = document.getElementById("profile-status");
  if (profileStatus) profileStatus.textContent = CAREER_LOAD_ERROR;
  const careerConnectionNote = document.getElementById("careerConnectionNote");
  if (careerConnectionNote) careerConnectionNote.textContent = CAREER_LOAD_ERROR;
}

function clearCareerLoadFailure() {
  const banner = document.getElementById("careerLoadFallback");
  if (banner) {
    banner.style.display = "none";
  }
  const profileStatus = document.getElementById("profile-status");
  if (profileStatus?.textContent === CAREER_LOAD_ERROR) {
    profileStatus.textContent = "";
  }
  const careerConnectionNote = document.getElementById("careerConnectionNote");
  if (careerConnectionNote?.textContent === CAREER_LOAD_ERROR) {
    careerConnectionNote.textContent = "";
  }
}

async function safeInvoke(command, args = {}, options = {}) {
  const {
    fallback = null,
    rethrow = false,
    silent = false,
  } = options;

  try {
    const shouldRedact = ["auth_login", "auth_register", "auth_reset_password_with_recovery_code"].includes(command);
    if (shouldRedact) {
      const safeArgs = { ...(args || {}) };
      for (const key of ["password", "passwordConfirm", "password_confirm"]) {
        if (key in safeArgs) safeArgs[key] = "[REDACTED]";
      }
      console.log("[invoke:start]", command, safeArgs);
    } else {
      console.log("[invoke:start]", command, args);
    }
    const result = await tauriInvoke(command, args);
    console.log("[invoke:ok]", command, result);
    return result;
  } catch (error) {
    console.error("[invoke:fail]", command, error);
    if (!silent && ["hub_get_mode", "career_get_status", "career_get_overview"].includes(command)) {
      showCareerLoadFailure(error);
    }
    if (rethrow) throw error;
    return fallback;
  }
}

const invokeStrict = (command, args = {}) => safeInvoke(command, args, { rethrow: true, silent: true });
const invoke = (command, args = {}) =>
  safeInvoke(command, args, {
    rethrow: true,
    silent: !["hub_get_mode", "career_get_status", "career_get_overview"].includes(command),
  });

window.addEventListener("error", (event) => {
  const error = event.error || (typeof event.message === "string" && event.message ? event.message : null);
  console.error("[ui:error]", error || event);
  if (error) {
    showCareerLoadFailure(error);
  }
});

window.addEventListener("unhandledrejection", (event) => {
  console.error("[ui:rejection]", event.reason);
  showCareerLoadFailure(event.reason);
});

window.invoke = invoke;
window.applySetting = (...args) => applySetting(...args);

function formatTelemetryNumber(value, digits = 0) {
  return Number(value ?? 0).toLocaleString(undefined, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

function formatCurrency(value) {
  return `EUR ${formatTelemetryNumber(value ?? 0, 0)}`;
}

function formatDistance(value) {
  return `${formatTelemetryNumber(value ?? 0, 1)} km`;
}

function formatDurationCompact(value) {
  const totalSeconds = Math.max(0, Number(value ?? 0));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  }
  return `${minutes}m`;
}

function formatDateTime(value) {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function humanizeToken(value) {
  return String(value ?? "")
    .replaceAll(/[_-]+/g, " ")
    .replaceAll(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (match) => match.toUpperCase());
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;")
    .replaceAll("'", "&#39;");
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
  if (!hasTauri) {
    let text = String(key ?? "");
    for (const [k, v] of Object.entries(params)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
    return text;
  }
  try {
    let text = await tauriInvoke("translate_command", { key });
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
    await safeInvoke("log_user_action", { action, stage }, { silent: true });
  } catch (error) {
    console.warn("User log failed:", error);
  }
}

window.logUserAction = logUserAction;

document.addEventListener("DOMContentLoaded", async () => {
  try {
    console.log("[ui] boot start");

    if (hasTauri) {
      try {
        ({ applySetting } = await import("./js/applySetting.js"));
      } catch (error) {
        console.warn("[ui] applySetting module load failed", error);
      }

      try {
        ({ checkUpdaterOnStartup, manualUpdateCheck } = await import("./js/updater.js"));
      } catch (error) {
        console.warn("[ui] updater module load failed", error);
      }
    }

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
    userMenuBtn: document.getElementById("userMenuBtn"),
    userMenuLabel: document.getElementById("userMenuLabel"),
    userMenuDropdown: document.getElementById("userMenuDropdown"),
    userMenuIdentity: document.getElementById("userMenuIdentity"),
    userMenuRole: document.getElementById("userMenuRole"),
    userMenuLogin: document.getElementById("userMenuLogin"),
    userMenuLogout: document.getElementById("userMenuLogout"),
    userMenuAdmin: document.getElementById("userMenuAdmin"),
    careerSidebarBalance: document.getElementById("careerSidebarBalance"),
    careerSidebarCompany: document.getElementById("careerSidebarCompany"),
    careerDashboardShell: document.querySelector(".career-dashboard-shell"),
    careerDetailHost: document.getElementById("careerDetailHost"),
    careerAuthGate: document.getElementById("careerAuthGate"),
    careerAuthStatus: document.getElementById("careerAuthStatus"),
    careerLoginView: document.getElementById("careerLoginView"),
    careerOnboardingView: document.getElementById("careerOnboardingView"),
    careerLoginEmail: document.getElementById("careerLoginEmail"),
    careerLoginPassword: document.getElementById("careerLoginPassword"),
    careerLoginSubmit: document.getElementById("careerLoginSubmit"),
    careerLoginCancel: document.getElementById("careerLoginCancel"),
    careerLoginError: document.getElementById("careerLoginError"),
    careerAuthLoginTab: document.getElementById("careerAuthLoginTab"),
    careerAuthRegisterTab: document.getElementById("careerAuthRegisterTab"),
    careerAuthLoginPanel: document.getElementById("careerAuthLoginPanel"),
    careerForgotPasswordGate: document.getElementById("careerForgotPasswordGate"),
    careerForgotPasswordOpen: document.getElementById("careerForgotPasswordOpen"),
    careerAuthResetPanel: document.getElementById("careerAuthResetPanel"),
    careerResetEmail: document.getElementById("careerResetEmail"),
    careerResetRecoveryCode: document.getElementById("careerResetRecoveryCode"),
    careerResetNewPassword: document.getElementById("careerResetNewPassword"),
    careerResetPasswordConfirm: document.getElementById("careerResetPasswordConfirm"),
    careerResetError: document.getElementById("careerResetError"),
    careerResetSubmit: document.getElementById("careerResetSubmit"),
    careerResetCancel: document.getElementById("careerResetCancel"),
    careerAuthRegisterPanel: document.getElementById("careerAuthRegisterPanel"),
    careerRegisterUsername: document.getElementById("careerRegisterUsername"),
    careerRegisterEmail: document.getElementById("careerRegisterEmail"),
    careerRegisterPassword: document.getElementById("careerRegisterPassword"),
    careerRegisterPasswordConfirm: document.getElementById("careerRegisterPasswordConfirm"),
    careerRegisterConsentPrivacy: document.getElementById("careerRegisterConsentPrivacy"),
    careerRegisterConsentTerms: document.getElementById("careerRegisterConsentTerms"),
    careerRegisterSubmit: document.getElementById("careerRegisterSubmit"),
    careerRegisterError: document.getElementById("careerRegisterError"),
    careerOnboardingJoinTab: document.getElementById("careerOnboardingJoinTab"),
    careerOnboardingCreateTab: document.getElementById("careerOnboardingCreateTab"),
    careerOnboardingJoinView: document.getElementById("careerOnboardingJoinView"),
    careerOnboardingCreateView: document.getElementById("careerOnboardingCreateView"),
    careerCompanySearch: document.getElementById("careerCompanySearch"),
    careerCompanyList: document.getElementById("careerCompanyList"),
    careerCompanyListEmpty: document.getElementById("careerCompanyListEmpty"),
    careerCompanyName: document.getElementById("careerCompanyName"),
    careerCompanyLocation: document.getElementById("careerCompanyLocation"),
    careerCompanyLanguage: document.getElementById("careerCompanyLanguage"),
    careerCompanyGame: document.getElementById("careerCompanyGame"),
    careerCompanyLogo: document.getElementById("careerCompanyLogo"),
    careerCompanyHeader: document.getElementById("careerCompanyHeader"),
    careerCompanyDescription: document.getElementById("careerCompanyDescription"),
    careerCompanyCreateSubmit: document.getElementById("careerCompanyCreateSubmit"),
    careerCompanyCreateError: document.getElementById("careerCompanyCreateError"),
    careerHeroTitle: document.getElementById("careerHeroTitle"),
    careerGameLabel: document.getElementById("careerGameLabel"),
    careerConnectionNote: document.getElementById("careerConnectionNote"),
    careerCompanyValue: document.getElementById("careerCompanyValue"),
    careerBalanceValue: document.getElementById("careerBalanceValue"),
    careerReputationValue: document.getElementById("careerReputationValue"),
    careerFleetStatusValue: document.getElementById("careerFleetStatusValue"),
    careerProfileAvatar: document.getElementById("careerProfileAvatar"),
    careerProfileName: document.getElementById("careerProfileName"),
    careerRoleBadge: document.getElementById("careerRoleBadge"),
    careerCompanyHeadline: document.getElementById("careerCompanyHeadline"),
    careerLevelValue: document.getElementById("careerLevelValue"),
    careerXpValue: document.getElementById("careerXpValue"),
    careerLevelProgressFill: document.getElementById("careerLevelProgressFill"),
    careerLevelProgressText: document.getElementById("careerLevelProgressText"),
    careerStatusGameRunning: document.getElementById("careerStatusGameRunning"),
    careerStatusPluginInstalled: document.getElementById("careerStatusPluginInstalled"),
    careerStatusSdkConnected: document.getElementById("careerStatusSdkConnected"),
    careerSpeedDial: document.getElementById("careerSpeedDial"),
    careerSpeedValue: document.getElementById("careerSpeedValue"),
    careerGearValue: document.getElementById("careerGearValue"),
    careerFuelValue: document.getElementById("careerFuelValue"),
    careerFuelPercent: document.getElementById("careerFuelPercent"),
    careerFuelBarFill: document.getElementById("careerFuelBarFill"),
    careerRpmValue: document.getElementById("careerRpmValue"),
    careerJobCard: document.getElementById("careerJobCard"),
    careerJobStatusPill: document.getElementById("careerJobStatusPill"),
    careerJobRoute: document.getElementById("careerJobRoute"),
    careerJobCompanies: document.getElementById("careerJobCompanies"),
    careerJobCargo: document.getElementById("careerJobCargo"),
    careerJobDistance: document.getElementById("careerJobDistance"),
    careerJobIncome: document.getElementById("careerJobIncome"),
    careerJobTimeRemaining: document.getElementById("careerJobTimeRemaining"),
    careerJobId: document.getElementById("careerJobId"),
    careerRecentJobsList: document.getElementById("careerRecentJobsList"),
    careerRecentJobsEmpty: document.getElementById("careerRecentJobsEmpty"),
    careerActivityList: document.getElementById("careerActivityList"),
    careerActivityEmpty: document.getElementById("careerActivityEmpty"),
    careerJobsTotalValue: document.getElementById("careerJobsTotalValue"),
    careerJobsTotalIncomeValue: document.getElementById("careerJobsTotalIncomeValue"),
    careerJobsAverageDistanceValue: document.getElementById("careerJobsAverageDistanceValue"),
    careerJobsSuccessRateValue: document.getElementById("careerJobsSuccessRateValue"),
    careerLiveRevenueValue: document.getElementById("careerLiveRevenueValue"),
    careerCostFuelValue: document.getElementById("careerCostFuelValue"),
    careerCostRepairValue: document.getElementById("careerCostRepairValue"),
    careerCostTollValue: document.getElementById("careerCostTollValue"),
    careerDriversOnlineValue: document.getElementById("careerDriversOnlineValue"),
    careerDriversRestingValue: document.getElementById("careerDriversRestingValue"),
    careerActiveTripValue: document.getElementById("careerActiveTripValue"),
    careerActiveTripSummary: document.getElementById("careerActiveTripSummary"),
    careerLogbookTable: document.getElementById("careerLogbookTable"),
    careerLogbookEmpty: document.getElementById("careerLogbookEmpty"),
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
    engineOn: false,
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
    engineOn: await t("career.status.engine_on"),
    engineOff: await t("career.status.engine_off"),
  };
  const careerUi = {
    noActiveTrip: await t("career.dashboard.no_active_trip"),
    tripWaiting: await t("career.dashboard.trip_waiting"),
    noLogbook: await t("career.jobs_logbook.empty"),
    noActiveJob: await t("career.jobs.active.none"),
    jobActive: await t("career.jobs.active.status_active"),
    jobNone: await t("career.jobs.active.status_none"),
    noContracts: await t("career.orders.none"),
    noJobs: await t("career.jobs.none"),
    currentJob: await t("career.jobs.current"),
    progress: await t("career.jobs.progress"),
    refreshJobs: await t("career.jobs.refresh"),
    acceptJob: await t("career.jobs.accept"),
    acceptedJob: await t("career.jobs.accepted"),
    noFreight: await t("career.freight.none"),
    noDispatcherEvents: await t("career.dispatcher.no_events"),
    dispatcherPriorityHigh: await t("career.dispatcher.priority_high"),
    dispatcherPriorityMedium: await t("career.dispatcher.priority_medium"),
    joinCompanyButton: await t("career.onboarding.join_button"),
    totalStaff: await t("career.members.total_staff"),
    systemControlled: await t("career.settings.system_controlled"),
    modulesValue: await t("career.settings.modules_value"),
    activeContracts: await t("career.company.active_contracts"),
    companyValue: await t("career.statistics.company_value"),
    speeding: await t("career.statistics.speeding"),
    noData: await t("career.shared.none"),
    player: await t("career.shared.player"),
  };
  let careerOverview = null;
  let careerOnboardingState = null;
  let cachedCompanyList = [];
  let cachedCompanyListAt = 0;
  const COMPANY_LIST_CACHE_MS = 15_000;
  let activeOnboardingTab = "join";
  let activeAuthTab = "login";
  let invalidPasswordAttempts = 0;
  let pendingRecoveryCodes = null;
  let lastStableActiveJob = null;
  let lastStableActiveJobAt = 0;
  const ACTIVE_JOB_EMPTY_DEBOUNCE_MS = 1500;
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

  const isCareerModeActive = () => document.body.classList.contains("mode-career");

  const setCareerAuthGateVisible = (visible) => {
    if (!refs.careerAuthGate) return;
    refs.careerAuthGate.hidden = !visible;
  };

  const showCareerLoginView = () => {
    setCareerAuthGateVisible(true);
    if (refs.careerLoginView) refs.careerLoginView.hidden = false;
    if (refs.careerOnboardingView) refs.careerOnboardingView.hidden = true;
    setInlineError(refs.careerLoginError, "");
    setInlineError(refs.careerRegisterError, "");
    applyAuthTab(activeAuthTab);
  };

  const showCareerOnboardingView = () => {
    setCareerAuthGateVisible(true);
    if (refs.careerLoginView) refs.careerLoginView.hidden = true;
    if (refs.careerOnboardingView) refs.careerOnboardingView.hidden = false;
  };

  const hideCareerAuthGate = () => {
    setCareerAuthGateVisible(false);
    if (refs.careerLoginError) refs.careerLoginError.hidden = true;
    if (refs.careerRegisterError) refs.careerRegisterError.hidden = true;
    if (refs.careerCompanyCreateError) refs.careerCompanyCreateError.hidden = true;
    if (refs.careerAuthStatus) refs.careerAuthStatus.hidden = true;
  };

  const setCareerAuthStatus = (message) => {
    if (!refs.careerAuthStatus) return;
    if (!message) {
      refs.careerAuthStatus.hidden = true;
      refs.careerAuthStatus.textContent = "";
      return;
    }
    refs.careerAuthStatus.hidden = false;
    refs.careerAuthStatus.textContent = message;
  };

  const setInlineError = (element, message) => {
    if (!element) return;
    if (!message) {
      element.hidden = true;
      element.textContent = "";
      return;
    }
    element.hidden = false;
    element.textContent = message;
  };

  const setUserMenuOpen = (open) => {
    if (!refs.userMenuDropdown || !refs.userMenuBtn) return;
    const visible = Boolean(open);
    refs.userMenuDropdown.hidden = !visible;
    refs.userMenuBtn.setAttribute("aria-expanded", visible ? "true" : "false");
  };

  const toggleUserMenu = () => setUserMenuOpen(refs.userMenuDropdown?.hidden);

  const updateUserMenu = async () => {
    const user = window.careerAuthUser || null;
    const labelKey = user ? "career.user_menu.account" : "career.user_menu.login";
    if (refs.userMenuLabel) refs.userMenuLabel.textContent = await t(labelKey);

    if (refs.userMenuIdentity) {
      refs.userMenuIdentity.textContent = user ? (user.email || user.username || "-") : await t("career.user_menu.not_logged_in");
    }
    if (refs.userMenuRole) {
      refs.userMenuRole.textContent = user ? (user.role || "") : "";
    }

    if (refs.userMenuLogin) refs.userMenuLogin.hidden = Boolean(user);
    if (refs.userMenuLogout) refs.userMenuLogout.hidden = !Boolean(user);
    if (refs.userMenuAdmin) refs.userMenuAdmin.hidden = !(user && String(user.role || "").toLowerCase() === "admin");
  };

  const normalizeCompanySearch = (value) => String(value || "").trim().toLowerCase();

  const renderCompanyList = (companies, query) => {
    if (!refs.careerCompanyList || !refs.careerCompanyListEmpty) return;
    const q = normalizeCompanySearch(query);
    const filtered = companies.filter((company) => {
      if (!q) return true;
      return String(company.name || "").toLowerCase().includes(q);
    });

    refs.careerCompanyListEmpty.hidden = filtered.length > 0;
    refs.careerCompanyList.innerHTML = filtered.map((company) => `
      <div class="career-company-item">
        <strong>${escapeHtml(company.name || "-")}</strong>
        <p>${escapeHtml(`${company.location || "-"} | ${(company.game || company.jobType || "-")}`)}</p>
        <button class="table-action" type="button" data-career-company-join="${escapeHtml(String(company.id))}">${escapeHtml(careerUi.joinCompanyButton)}</button>
      </div>
    `).join("");
  };

  const loadCompanyList = async ({ force = false } = {}) => {
    const now = Date.now();
    if (!force && cachedCompanyList.length && now - cachedCompanyListAt < COMPANY_LIST_CACHE_MS) {
      return cachedCompanyList;
    }

    const companies = await invoke("company_list", { limit: 50 });
    cachedCompanyList = Array.isArray(companies) ? companies : [];
    cachedCompanyListAt = now;
    return cachedCompanyList;
  };

  const applyOnboardingTab = (tab) => {
    activeOnboardingTab = tab === "create" ? "create" : "join";
    if (refs.careerOnboardingJoinTab) refs.careerOnboardingJoinTab.classList.toggle("active", activeOnboardingTab === "join");
    if (refs.careerOnboardingCreateTab) refs.careerOnboardingCreateTab.classList.toggle("active", activeOnboardingTab === "create");
    if (refs.careerOnboardingJoinView) refs.careerOnboardingJoinView.hidden = activeOnboardingTab !== "join";
    if (refs.careerOnboardingCreateView) refs.careerOnboardingCreateView.hidden = activeOnboardingTab !== "create";
  };

  const applyAuthTab = (tab) => {
    activeAuthTab = tab === "register" ? "register" : "login";
    if (refs.careerAuthLoginTab) refs.careerAuthLoginTab.classList.toggle("active", activeAuthTab === "login");
    if (refs.careerAuthRegisterTab) refs.careerAuthRegisterTab.classList.toggle("active", activeAuthTab === "register");
    if (refs.careerAuthLoginPanel) refs.careerAuthLoginPanel.hidden = activeAuthTab !== "login";
    if (refs.careerAuthRegisterPanel) refs.careerAuthRegisterPanel.hidden = activeAuthTab !== "register";
    if (refs.careerAuthResetPanel) refs.careerAuthResetPanel.hidden = true;
    setInlineError(refs.careerResetError, "");
  };

  const showPasswordResetPanel = () => {
    if (refs.careerAuthLoginPanel) refs.careerAuthLoginPanel.hidden = true;
    if (refs.careerAuthRegisterPanel) refs.careerAuthRegisterPanel.hidden = true;
    if (refs.careerAuthResetPanel) refs.careerAuthResetPanel.hidden = false;
    setInlineError(refs.careerResetError, "");
    if (refs.careerResetEmail) refs.careerResetEmail.value = refs.careerLoginEmail?.value?.trim() || "";
    if (refs.careerResetRecoveryCode) refs.careerResetRecoveryCode.value = "";
    if (refs.careerResetNewPassword) refs.careerResetNewPassword.value = "";
    if (refs.careerResetPasswordConfirm) refs.careerResetPasswordConfirm.value = "";
  };

  const showLoginPanel = () => {
    applyAuthTab("login");
  };

  const readFileBase64 = async (input) => {
    const file = input?.files?.[0];
    if (!file) return { base64: null, mime: null };

    const buf = await file.arrayBuffer();
    const bytes = new Uint8Array(buf);
    let binary = "";
    const chunkSize = 0x2000;
    for (let i = 0; i < bytes.length; i += chunkSize) {
      const end = Math.min(i + chunkSize, bytes.length);
      let chunk = "";
      for (let j = i; j < end; j++) {
        chunk += String.fromCharCode(bytes[j]);
      }
      binary += chunk;
    }
    return {
      base64: btoa(binary),
      mime: file.type || null,
    };
  };

  const refreshCareerOnboardingGate = async () => {
    if (!isCareerModeActive()) {
      hideCareerAuthGate();
      return;
    }

    try {
      careerOnboardingState = await invoke("career_get_onboarding_state");
    } catch (error) {
      console.error("[career] onboarding state failed", error);
      showCareerLoginView();
      setCareerAuthStatus(await t("career.auth.state_failed"));
      return;
    }

    if (careerOnboardingState?.hasCompany) {
      hideCareerAuthGate();
      return;
    }

    if (!careerOnboardingState?.needsLogin) {
      try {
        window.careerAuthUser = await invoke("auth_get_current_user");
      } catch {
        window.careerAuthUser = null;
      }
    } else {
      window.careerAuthUser = null;
    }

    await updateUserMenu();

    if (refs.careerRoleBadge) {
      refs.careerRoleBadge.hidden = !careerOnboardingState?.hasCompany;
      refs.careerRoleBadge.textContent = careerOnboardingState?.isCompanyOwner ? await t("career.portal.role_owner") : await t("career.portal.role_member");
    }

    if (careerOnboardingState?.needsLogin) {
      showCareerLoginView();
      setCareerAuthStatus("");
      return;
    }

    if (careerOnboardingState?.needsCompany) {
      try {
        const currentCompany = await invoke("company_get_current");
        if (currentCompany) {
          hideCareerAuthGate();
          return;
        }
      } catch {}
      showCareerOnboardingView();
      setCareerAuthStatus("");
      applyOnboardingTab(activeOnboardingTab);
      const companies = await loadCompanyList({ force: false });
      renderCompanyList(companies, refs.careerCompanySearch?.value || "");
      return;
    }

    hideCareerAuthGate();
  };

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
  const jobStatusClass = (status) => {
    const normalized = String(status || "").toLowerCase();
    if (normalized === "active") return "active";
    if (normalized === "completed") return "completed";
    if (normalized === "aborted" || normalized === "cancelled" || normalized === "canceled") return "cancelled";
    return "unknown";
  };
  const renderLogbookRows = (jobs) => jobs.slice(0, 40).map((job, index) => {
    const started = formatDateTime(job.startedAtUtc);
    const game = String(careerState.activeGame || lastSelectedGame || "ets2").toUpperCase();
    const route = `${job.originCity || "--"} -> ${job.destinationCity || "--"}`;
    const statusLabel = humanizeToken(job.status);
    const statusClass = jobStatusClass(job.status);

    return `
      <div class="table-row career-logbook-row" data-job-id="${escapeHtml(job.jobId || "")}">
        <span class="cell cell-index">${String(index + 1).padStart(2, "0")}</span>
        <span class="cell cell-date">${escapeHtml(started)}</span>
        <span class="cell cell-game">${escapeHtml(game)}</span>
        <span class="cell cell-cargo">${escapeHtml(job.cargo || careerUi.noData)}</span>
        <span class="cell cell-route">${escapeHtml(route)}</span>
        <span class="cell cell-distance">${escapeHtml(formatDistance(job.plannedDistanceKm || 0))}</span>
        <span class="cell cell-income">${escapeHtml(formatCurrency(job.income ?? 0))}</span>
        <span class="cell cell-status"><span class="status-badge status-${statusClass}">${escapeHtml(statusLabel)}</span></span>
      </div>
    `;
  }).join("");

  const renderTripLogbookRows = (trips) => trips.slice(0, 40).map((trip) => {
    const startedAt =
      trip?.startedAtUtc ??
      trip?.startedAt ??
      trip?.startedUtc ??
      trip?.timestampUtc ??
      trip?.timestamp ??
      null;
    const started = formatDateTime(startedAt);
    const origin = trip?.origin ?? trip?.originCity ?? "--";
    const destination = trip?.destination ?? trip?.destinationCity ?? "--";
    const distanceKmRaw = trip?.distanceKm ?? trip?.distance_km ?? trip?.distance ?? null;
    const distanceKm = distanceKmRaw === null || distanceKmRaw === undefined ? null : Number(distanceKmRaw);

    return `
      <div class="table-row career-tripbook-row">
        <span class="cell cell-date">${escapeHtml(started)}</span>
        <span class="cell cell-origin">${escapeHtml(origin)}</span>
        <span class="cell cell-destination">${escapeHtml(destination)}</span>
        <span class="cell cell-distance">${escapeHtml(distanceKm === null || Number.isNaN(distanceKm) ? "-" : formatDistance(distanceKm))}</span>
      </div>
    `;
  }).join("");

  const renderCareerLogbook = () => {
    if (!refs.careerLogbookTable || !refs.careerLogbookEmpty) return;
    const jobs = careerOverview?.recentJobs || [];

    refs.careerLogbookEmpty.hidden = jobs.length > 0;
    refs.careerLogbookEmpty.textContent = careerUi.noLogbook;
    refs.careerLogbookTable.innerHTML = renderLogbookRows(jobs);
  };
  const renderActiveJobCard = () => {
    if (!refs.careerJobRoute || !refs.careerJobCompanies || !refs.careerJobCargo) return;
    const now = Date.now();
    const incoming = careerOverview?.activeJob || null;

    if (incoming) {
      lastStableActiveJob = incoming;
      lastStableActiveJobAt = now;
    } else if (!careerState.bridgeConnected) {
      lastStableActiveJob = null;
      lastStableActiveJobAt = 0;
    }

    const useStable =
      !incoming &&
      careerState.bridgeConnected &&
      lastStableActiveJob &&
      now - lastStableActiveJobAt < ACTIVE_JOB_EMPTY_DEBOUNCE_MS;

    const job = incoming || (useStable ? lastStableActiveJob : null);

    if (refs.careerJobCard) refs.careerJobCard.classList.toggle("is-stale", Boolean(useStable));

    if (!job) {
      if (refs.careerJobStatusPill) refs.careerJobStatusPill.textContent = careerUi.jobNone;
      refs.careerJobRoute.textContent = careerUi.noActiveJob;
      refs.careerJobCompanies.textContent = "";
      if (refs.careerJobCargo) refs.careerJobCargo.textContent = "-";
      if (refs.careerJobDistance) refs.careerJobDistance.textContent = "-";
      if (refs.careerJobIncome) refs.careerJobIncome.textContent = "-";
      if (refs.careerJobTimeRemaining) refs.careerJobTimeRemaining.textContent = "-";
      if (refs.careerJobId) refs.careerJobId.textContent = "-";
      return;
    }

    if (refs.careerJobStatusPill) refs.careerJobStatusPill.textContent = careerUi.jobActive;
    refs.careerJobRoute.textContent = `${job.originCity || careerUi.noData} -> ${job.destinationCity || careerUi.noData}`;
    refs.careerJobCompanies.textContent = `${job.sourceCompany || careerUi.noData} -> ${job.destinationCompany || careerUi.noData}`;
    if (refs.careerJobCargo) refs.careerJobCargo.textContent = job.cargo || careerUi.noData;
    if (refs.careerJobDistance) refs.careerJobDistance.textContent = formatDistance(job.plannedDistanceKm || 0);
    if (refs.careerJobIncome) refs.careerJobIncome.textContent = formatCurrency(job.income ?? 0);

    const remainingMin = typeof job.remainingTimeMin === "number" ? Math.max(0, job.remainingTimeMin) : null;
    if (refs.careerJobTimeRemaining) {
      refs.careerJobTimeRemaining.textContent = remainingMin === null ? "-" : formatDurationCompact(remainingMin * 60);
    }
    if (refs.careerJobId) refs.careerJobId.textContent = job.jobId || "-";
  };

  const renderRecentJobs = () => {
    if (!refs.careerRecentJobsList || !refs.careerRecentJobsEmpty) return;
    const jobs = careerOverview?.recentJobs || [];

    refs.careerRecentJobsEmpty.hidden = jobs.length > 0;
    refs.careerRecentJobsEmpty.textContent = careerUi.noLogbook;
    refs.careerRecentJobsList.innerHTML = jobs.slice(0, 6).map((job) => `
      <div class="job-history-item">
        <span class="job-metric-label">${escapeHtml(formatDateTime(job.startedAtUtc))}</span>
        <strong>${escapeHtml(`${job.originCity || "--"} -> ${job.destinationCity || "--"}`)}</strong>
        <p>${escapeHtml(`${job.cargo || careerUi.noData} | ${formatDistance(job.plannedDistanceKm || 0)} | ${formatCurrency(job.income ?? 0)} | ${humanizeToken(job.status)}`)}</p>
      </div>
    `).join("");
  };

  const getDispatcherEventTimestampMs = (event) => {
    const raw =
      event?.atUtc ??
      event?.at ??
      event?.timestampUtc ??
      event?.timestamp ??
      event?.createdAtUtc ??
      event?.createdAt ??
      event?.occurredAtUtc ??
      event?.occurredAt ??
      null;
    if (!raw) return null;
    const date = new Date(raw);
    const ms = date.getTime();
    return Number.isNaN(ms) ? null : ms;
  };

  const renderCareerActivityFeed = () => {
    if (!refs.careerActivityList || !refs.careerActivityEmpty) return;
    const events = Array.isArray(careerOverview?.dispatcherEvents) ? careerOverview.dispatcherEvents : [];
    const limit = 8;

    let display = events.slice();
    const hasTimestamps = display.some((event) => getDispatcherEventTimestampMs(event) !== null);
    if (hasTimestamps) {
      display.sort((a, b) => (getDispatcherEventTimestampMs(b) ?? 0) - (getDispatcherEventTimestampMs(a) ?? 0));
    }
    display = display.slice(0, limit);

    refs.careerActivityEmpty.hidden = display.length > 0;
    refs.careerActivityList.hidden = display.length === 0;
    refs.careerActivityEmpty.textContent = careerUi.noDispatcherEvents;

    refs.careerActivityList.innerHTML = display.map((event) => {
      const severity = String(event?.severity ?? "medium").toLowerCase();
      const severityClass = severity === "high" ? "high" : severity === "medium" ? "medium" : "medium";
      const severityLabel = severityClass === "high" ? careerUi.dispatcherPriorityHigh : careerUi.dispatcherPriorityMedium;
      const timeRaw =
        event?.atUtc ??
        event?.at ??
        event?.timestampUtc ??
        event?.timestamp ??
        event?.createdAtUtc ??
        event?.createdAt ??
        event?.occurredAtUtc ??
        event?.occurredAt ??
        null;
      const timeText = timeRaw ? formatDateTime(timeRaw) : "";

      return `
        <article class="career-activity-item">
          <div class="career-activity-head">
            <span class="career-activity-priority priority ${escapeHtml(severityClass)}">${escapeHtml(severityLabel)}</span>
            ${timeText ? `<span class="career-activity-time muted">${escapeHtml(timeText)}</span>` : ""}
          </div>
          <strong class="career-activity-title">${escapeHtml(event?.title || careerUi.noData)}</strong>
          ${event?.impact ? `<p class="career-activity-impact">${escapeHtml(event.impact)}</p>` : ""}
        </article>
      `;
    }).join("");
  };

  const renderCareerDetailPanel = async (panel) => {
    const overview = careerOverview;
    const fallbackCompanyName = refs.profileNameDisplay?.textContent?.trim() || uiText.noCompany;
    const companyName = overview?.economy?.companyName || fallbackCompanyName;
    const money = overview?.bank?.cashBalance ?? Number(window.currentProfileData?.money ?? 0);
    const xp = overview?.reputation?.xpPoints ?? Number(window.currentProfileData?.xp ?? 0);
    const level = overview?.reputation?.level ?? deriveLevel(xp);
    const staff = overview?.employees || [];
    const totalSalary = staff.reduce((sum, member) => sum + Number(member.salary ?? 0), 0);
    const leadDriver = staff.find((member) => String(member.role).toLowerCase() === "driver") || staff[0] || null;
    const employeeOverview = overview?.employeeOverview || {
      total: staff.length,
      onDuty: 0,
      resting: 0,
      dispatchers: 0,
    };
    const fleetAssets = overview?.fleet || [];
    const fleetOverview = overview?.fleetOverview || {
      trucks: window.allTrucks?.length || 0,
      trailers: window.allTrailers?.length || 0,
      playerCondition: 0,
    };
    const contractRows = (overview?.contracts || []).filter((contract) => contract.active);
    const jobs = overview?.jobs || [];
    const currentJob = overview?.currentJob || null;
    const freightOffers = overview?.freightOffers || [];
    const dispatcherEvents = overview?.dispatcherEvents || [];
    const trips = overview?.recentTrips || [];
    const dashboard = overview?.dashboard || {
      fuelCost: 0,
      repairCost: 0,
      tollCost: 0,
      driversOnline: 0,
      driversResting: 0,
    };
    const statistics = overview?.statistics || {
      totalIncome: 0,
      totalKilometers: 0,
      speedingEvents: 0,
      companyValue: money,
    };

    switch (panel) {
      case "members":
        return buildSectionFrame(
          "career.nav.members",
          "career.members.title",
          "career.members.summary",
          buildDetailCards([
            {
              label: await t("career.members.lead_driver"),
              value: leadDriver?.name || careerUi.noData,
              copy: leadDriver ? `${humanizeToken(leadDriver.status)} | ${formatCurrency(leadDriver.salary)}` : "",
            },
            {
              label: await t("career.members.dispatchers"),
              value: String(employeeOverview.dispatchers ?? 0).padStart(2, "0"),
              copy: formatCurrency(
                staff
                  .filter((member) => String(member.role).toLowerCase() === "dispatcher")
                  .reduce((sum, member) => sum + Number(member.salary ?? 0), 0)
              ),
            },
            {
              label: careerUi.totalStaff,
              value: String(employeeOverview.total ?? staff.length).padStart(2, "0"),
              copy: formatCurrency(totalSalary),
            },
          ])
        );
      case "orders":
        return buildSectionFrame(
          "career.nav.orders",
          "career.orders.title",
          "career.orders.summary",
          `
            <div class="filters-row">
              <button class="table-action" data-career-action="generate-jobs">${careerUi.refreshJobs}</button>
            </div>
            <div class="detail-grid">
              <article class="detail-card">
                <span>${careerUi.currentJob}</span>
                <strong>${escapeHtml(currentJob ? `${currentJob.source} - ${currentJob.destination}` : careerUi.noJobs)}</strong>
                <p>${escapeHtml(currentJob ? `${currentJob.cargo} | ${formatDistance(currentJob.progressKm)} / ${formatDistance(currentJob.distanceKm)}` : careerUi.tripWaiting)}</p>
              </article>
            </div>
            <div class="table-shell">
              <div class="table-row table-head">
                <span>${await t("career.orders.origin")}</span>
                <span>${await t("career.orders.destination")}</span>
                <span>${await t("career.orders.cargo")}</span>
                <span>${await t("career.orders.payout")}</span>
                <span>${careerUi.progress}</span>
                <span>${await t("career.freight.accept")}</span>
              </div>
              ${(jobs.length ? jobs : [null]).map((job) => job
                ? `
                  <div class="table-row">
                    <span>${escapeHtml(job.source)}</span>
                    <span>${escapeHtml(job.destination)}</span>
                    <span>${escapeHtml(job.cargo)}</span>
                    <span>${escapeHtml(formatCurrency(job.estimatedPayout))}</span>
                    <span>${escapeHtml(`${formatDistance(job.progressKm)} / ${formatDistance(job.distanceKm)}`)}</span>
                    <button
                      class="table-action"
                      data-career-action="accept-job"
                      data-job-id="${escapeHtml(job.id)}"
                      ${job.accepted ? "disabled" : ""}
                    >
                      ${job.accepted ? careerUi.acceptedJob : careerUi.acceptJob}
                    </button>
                  </div>
                `
                : `
                  <div class="table-row">
                    <span>${escapeHtml(careerUi.noJobs)}</span>
                    <span>-</span>
                    <span>-</span>
                    <span>-</span>
                    <span>-</span>
                    <button class="table-action" disabled>${careerUi.acceptJob}</button>
                  </div>
                `).join("")}
            </div>
          `
        );
      case "logbook":
        return buildSectionFrame(
          "career.nav.logbook",
          "career.logbook.title",
          "career.logbook.summary",
          `
            <div class="table-shell career-tripbook-table">
              <div class="table-row table-head career-tripbook-head">
                <span>${await t("career.logbook.started")}</span>
                <span>${await t("career.orders.origin")}</span>
                <span>${await t("career.orders.destination")}</span>
                <span>${await t("career.logbook.distance")}</span>
              </div>
              ${trips.length ? renderTripLogbookRows(trips) : `<p class="career-logbook-empty">${escapeHtml(careerUi.noLogbook)}</p>`}
            </div>
          `
        );
      case "freight":
        {
          const freightAcceptLabel = await t("career.freight.accept");
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
                    <span>${freightAcceptLabel}</span>
                  </div>
                  ${(freightOffers.length ? freightOffers : [null]).slice(0, 3).map((offer) => offer
                    ? `
                      <div class="table-row">
                        <span>${escapeHtml(`${offer.origin} - ${offer.destination}`)}</span>
                        <span>${escapeHtml(formatCurrency(offer.payout))}</span>
                        <span>${escapeHtml(`${humanizeToken(offer.risk)} | ${offer.etaHours}h`)}</span>
                        <button class="table-action" disabled>${freightAcceptLabel}</button>
                      </div>
                    `
                    : `
                      <div class="table-row">
                        <span>${escapeHtml(careerUi.noFreight)}</span>
                        <span>-</span>
                        <span>-</span>
                        <button class="table-action" disabled>${freightAcceptLabel}</button>
                      </div>
                    `).join("")}
                </div>
              </div>
              <div class="map-preview"><div class="map-grid"><span></span></div></div>
            </div>
          `
          );
        }
      case "dispatcher":
        {
          const eventCards = [];
          for (const event of dispatcherEvents.slice(0, 3)) {
            eventCards.push({
              label: await t(event.severity === "high" ? "career.dispatcher.priority_high" : "career.dispatcher.priority_medium"),
              value: event.title,
              copy: event.impact,
            });
          }
        return buildSectionFrame(
          "career.nav.dispatcher",
          "career.dispatcher.title",
          "career.dispatcher.summary",
          buildDetailCards(
            eventCards.length
              ? eventCards
              : [{ label: await t("career.dispatcher.priority_medium"), value: careerUi.noDispatcherEvents, copy: careerUi.tripWaiting }]
          )
        );
        }
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
                  <strong>${escapeHtml(careerOverview?.activeTrip ? `${careerOverview.activeTrip.origin} - ${careerOverview.activeTrip.destination}` : careerUi.noActiveTrip)}</strong>
                  <p>${escapeHtml(careerOverview?.activeTrip ? `${formatDistance(careerOverview.activeTrip.distanceKm)} | ${formatDurationCompact(careerOverview.activeTrip.durationSeconds)}` : careerUi.tripWaiting)}</p>
                </article>
                <article class="detail-card">
                  <span>${await t("career.livemap.convoy_status")}</span>
                  <strong>${escapeHtml(careerState.bridgeConnected ? uiText.bridgeOnline : uiText.bridgeOffline)}</strong>
                  <p>${escapeHtml(careerState.bridgeConnected ? careerText.live : careerText.waiting)}</p>
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
              ${(fleetAssets.length ? fleetAssets : [null]).slice(0, 4).map((asset) => asset
                ? `
                  <div class="table-row">
                    <span>${escapeHtml(`${asset.brand} ${asset.model}`)}</span>
                    <span>${escapeHtml(`${formatTelemetryNumber(asset.conditionPercent, 0)}%`)}</span>
                    <span>${escapeHtml(asset.status === "player" ? careerUi.player : humanizeToken(asset.status))}</span>
                    <span>${escapeHtml(formatDistance(asset.serviceDueKm))}</span>
                  </div>
                `
                : `
                  <div class="table-row">
                    <span>${escapeHtml(careerUi.noData)}</span>
                    <span>-</span>
                    <span>-</span>
                    <span>-</span>
                  </div>
                `).join("")}
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
              { label: await t("career.balance.fuel"), value: formatCurrency(dashboard.fuelCost) },
              { label: await t("career.balance.repairs"), value: formatCurrency(dashboard.repairCost) },
              { label: await t("career.balance.salaries"), value: formatCurrency(totalSalary) },
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
            { label: await t("career.statistics.profit"), value: formatCurrency(statistics.totalIncome) },
            { label: await t("career.statistics.kilometers"), value: formatDistance(statistics.totalKilometers) },
            { label: careerUi.speeding, value: String(statistics.speedingEvents ?? 0) },
            { label: careerUi.companyValue, value: formatCurrency(statistics.companyValue ?? 0) },
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
            { label: await t("career.achievements.reputation"), value: overview?.reputation?.label || `L${level}` },
          ])
        );
      case "company":
        return buildSectionFrame(
          "career.nav.company",
          "career.company.title",
          "career.company.summary",
          buildDetailCards([
            { label: await t("career.company.headquarters"), value: companyName },
            { label: await t("career.company.staff_capacity"), value: String(employeeOverview.total ?? 0) },
            { label: careerUi.activeContracts, value: String(contractRows.length) },
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
      case "account":
        {
          let accountOverview = null;
          try {
            accountOverview = await invoke("auth_get_account_overview");
          } catch (error) {
            console.error("[career] account overview failed", error);
          }

          const user = accountOverview?.user || null;
          if (!user) {
            return buildSectionFrame(
              "career.nav.account",
              "career.account.title",
              "career.account.summary",
              `<p class="career-account-empty muted">${escapeHtml(await t("career.account.not_logged_in"))}</p>`
            );
          }

          const currentSessionId = accountOverview?.currentSessionId ?? null;
          const sessions = Array.isArray(accountOverview?.sessions) ? accountOverview.sessions : [];
          const sessionsMarkup = sessions.length
            ? `
              <div class="table-shell career-account-sessions">
                <div class="table-row table-head">
                  <span>${await t("career.account.sessions.created_at")}</span>
                  <span>${await t("career.account.sessions.last_used_at")}</span>
                  <span>${await t("career.account.sessions.expires_at")}</span>
                  <span>${await t("career.account.sessions.current")}</span>
                </div>
                <div class="table-scroll">
                  ${sessions.map((session) => {
                    const isCurrent = currentSessionId !== null && session.id === currentSessionId;
                    const createdAt = formatDateTime(session.createdAt);
                    const lastUsedAt = session.lastUsedAt ? formatDateTime(session.lastUsedAt) : "-";
                    const expiresAt = session.expiresAt ? formatDateTime(session.expiresAt) : "-";
                    return `
                      <div class="table-row ${isCurrent ? "is-current" : ""}">
                        <span>${escapeHtml(createdAt)}</span>
                        <span>${escapeHtml(lastUsedAt)}</span>
                        <span>${escapeHtml(expiresAt)}</span>
                        <span>${isCurrent ? "✓" : ""}</span>
                      </div>
                    `;
                  }).join("")}
                </div>
              </div>
            `
            : `<p class="muted">${escapeHtml(await t("career.account.sessions.empty"))}</p>`;

          const codesToShow = pendingRecoveryCodes;
          pendingRecoveryCodes = null;
          const recoveryCodesMarkup = codesToShow && codesToShow.length
            ? `
              <div class="career-account-codes panel-surface">
                <div class="panel-head compact">
                  <div>
                    <span class="eyebrow">${await t("career.account.recovery_codes.eyebrow")}</span>
                    <h3>${await t("career.account.recovery_codes.generated_title")}</h3>
                  </div>
                </div>
                <ul class="career-account-code-list">
                  ${codesToShow.map((code) => `<li><code>${escapeHtml(code)}</code></li>`).join("")}
                </ul>
                <p class="muted">${escapeHtml(await t("career.account.recovery_codes.generated_hint"))}</p>
              </div>
            `
            : "";

          const unusedRecoveryCodes = Number(accountOverview?.unusedRecoveryCodes ?? 0);
          const mau = accountOverview?.mau || {};
          const mauMonth = String(mau.yearMonth || "");
          const installationActive = Boolean(mau.installationActive);
          const currentAccountActive = Boolean(mau.currentAccountActive);
          const activeAccounts = Number(mau.activeAccounts ?? 0);

          return buildSectionFrame(
            "career.nav.account",
            "career.account.title",
            "career.account.summary",
            `
              ${buildDetailCards([
                { label: await t("career.account.fields.username"), value: escapeHtml(user.username || "-") },
                { label: await t("career.account.fields.email"), value: escapeHtml(user.email || "-") },
                { label: await t("career.account.fields.role"), value: escapeHtml(user.role || "-") },
              ])}

              <div class="divider"></div>

              <div class="detail-grid three-up">
                <article class="detail-card">
                  <span>${await t("career.account.mau.month")}</span>
                  <strong>${escapeHtml(mauMonth || "-")}</strong>
                </article>
                <article class="detail-card">
                  <span>${await t("career.account.mau.installation")}</span>
                  <strong>${installationActive ? await t("career.account.mau.active") : await t("career.account.mau.inactive")}</strong>
                </article>
                <article class="detail-card">
                  <span>${await t("career.account.mau.accounts")}</span>
                  <strong>${escapeHtml(String(activeAccounts).padStart(2, "0"))}</strong>
                  <p>${escapeHtml(currentAccountActive ? await t("career.account.mau.current_active") : await t("career.account.mau.current_inactive"))}</p>
                </article>
              </div>

              <div class="divider"></div>

              <div class="career-account-actions">
                <div class="career-account-action-row">
                  <span>${await t("career.account.recovery_codes.unused")}</span>
                  <strong>${String(unusedRecoveryCodes)}</strong>
                </div>
                <button class="table-action" type="button" data-career-action="generate-recovery-codes">${await t("career.account.recovery_codes.generate_button")}</button>
              </div>

              ${recoveryCodesMarkup}

              <div class="divider"></div>

              <div class="panel-subhead">
                <span class="card-label">${await t("career.account.sessions.title")}</span>
                <span class="card-hint">${await t("career.account.sessions.hint")}</span>
              </div>
              ${sessionsMarkup}
            `
          );
        }
      case "admin_db":
        {
          let overview = null;
          let errorText = "";
          try {
            overview = await invoke("auth_admin_get_db_overview");
          } catch (error) {
            errorText = await resolveAuthErrorMessage(error);
            console.error("[admin_db] load failed", error);
          }

          if (!overview) {
            return buildSectionFrame(
              "career.admin_db.eyebrow",
              "career.admin_db.title",
              "career.admin_db.summary",
              `<p class="career-account-empty muted">${escapeHtml(errorText || await t("career.admin_db.load_failed"))}</p>`
            );
          }

          const users = Array.isArray(overview.users) ? overview.users : [];
          const usersMarkup = users.length
            ? `
              <div class="table-shell career-admin-users">
                <div class="table-row table-head">
                  <span>${await t("career.admin_db.users.id")}</span>
                  <span>${await t("career.admin_db.users.email")}</span>
                  <span>${await t("career.admin_db.users.role")}</span>
                  <span>${await t("career.admin_db.users.created_at")}</span>
                  <span>${await t("career.admin_db.users.last_login_at")}</span>
                  <span>${await t("career.admin_db.users.session")}</span>
                </div>
                <div class="table-scroll">
                  ${users.map((user) => {
                    const createdAt = formatDateTime(user.createdAt);
                    const lastLogin = user.lastLoginAt ? formatDateTime(user.lastLoginAt) : "-";
                    const sessionLabel = user.hasActiveSession ? "✓" : "";
                    return `
                      <div class="table-row">
                        <span>${escapeHtml(String(user.id))}</span>
                        <span>${escapeHtml(user.email || "-")}</span>
                        <span>${escapeHtml(user.role || "-")}</span>
                        <span>${escapeHtml(createdAt)}</span>
                        <span>${escapeHtml(lastLogin)}</span>
                        <span>${sessionLabel}</span>
                      </div>
                    `;
                  }).join("")}
                </div>
              </div>
            `
            : `<p class="muted">${escapeHtml(await t("career.admin_db.users.empty"))}</p>`;

          return buildSectionFrame(
            "career.admin_db.eyebrow",
            "career.admin_db.title",
            "career.admin_db.summary",
            `
              ${buildDetailCards([
                { label: await t("career.admin_db.paths.db"), value: escapeHtml(overview.dbPath || "-") },
                { label: await t("career.admin_db.paths.session_file"), value: escapeHtml(overview.sessionFilePath || "-") },
                { label: await t("career.admin_db.users.total"), value: String(users.length) },
              ])}
              <div class="divider"></div>
              ${usersMarkup}
            `
          );
        }
      case "settings":
      default:
        return buildSectionFrame(
          "career.nav.settings",
          "career.settings.title",
          "career.settings.summary",
          buildDetailCards([
            { label: await t("career.settings.theme"), value: localStorage.getItem("theme") || "neon" },
            { label: await t("career.settings.language"), value: careerUi.systemControlled },
            { label: await t("career.settings.modules"), value: careerUi.modulesValue },
          ])
        );
    }
  };

  const setLamp = (element, active) => element?.classList.toggle("is-active", Boolean(active));
  const setCareerGame = (game) => {
    const label = (game || "ets2").toUpperCase();
    if (refs.careerHeroTitle) refs.careerHeroTitle.textContent = label;
    if (refs.careerGameLabel) {
      refs.careerGameLabel.textContent = `${label} | ${careerState.engineOn ? uiText.engineOn : uiText.engineOff}`;
    }
  };
  let activeCareerPanel = "dashboard";

  const setHubVisibility = (visible) => {
    refs.hubScreen?.classList.toggle("is-hidden", !visible);
  };

  const applyCareerState = () => {
    setLamp(refs.statusGameRunning, careerState.gameRunning);
    setLamp(refs.statusPluginInstalled, careerState.pluginInstalled);
    setLamp(refs.statusSdkConnected, careerState.bridgeConnected);
    setLamp(refs.careerStatusGameRunning, careerState.gameRunning);
    setLamp(refs.careerStatusPluginInstalled, careerState.pluginInstalled);
    setLamp(refs.careerStatusSdkConnected, careerState.bridgeConnected);
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
    if (!careerState.gameRunning) {
      refs.careerConnectionNote.textContent = careerText.gameStopped;
      return;
    }
    if (!careerState.pluginInstalled) {
      refs.careerConnectionNote.textContent = careerText.missingPlugin;
      return;
    }
    refs.careerConnectionNote.textContent = careerText.waiting;
  };

  const applyHubMode = (mode) => {
    const normalizedMode = mode === "utility" ? "editor" : mode;
    const isCareer = normalizedMode === "career";
    document.body.classList.toggle("mode-career", isCareer);
    document.body.classList.toggle("mode-editor", !isCareer);
    refs.editorModeBtn?.classList.toggle("active", !isCareer);
    refs.careerModeBtn?.classList.toggle("active", isCareer);
    void refreshCareerOnboardingGate();
  };

  const updateEditorStage = async (tab) => {
    const meta = editorStageMeta[tab] || editorStageMeta.profile;
    if (refs.editorStageTitle) refs.editorStageTitle.textContent = await t(meta.title);
    if (refs.editorStageSummary) refs.editorStageSummary.textContent = await t(meta.summary);
  };

  const updateOperationalOverview = () => {
    const overview = careerOverview;
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
    const companyLabel =
      overview?.economy?.companyName || (window.selectedProfilePath ? profileLabel : uiText.noCompany);

    const emptyMetric = "-";
    const asNumberOrNull = (value) => {
      if (value === null || value === undefined) return null;
      const number = Number(value);
      return Number.isNaN(number) ? null : number;
    };
    const setTextOrEmpty = (element, value, formatter = (v) => String(v)) => {
      if (!element) return;
      if (value === null || value === undefined) {
        element.textContent = emptyMetric;
        return;
      }
      element.textContent = formatter(value);
    };
    const formatTwoDigitOrEmpty = (value) => {
      const numeric = asNumberOrNull(value);
      if (numeric === null) return "--";
      return String(Math.max(0, Math.floor(numeric))).padStart(2, "0");
    };

    const cashBalance = asNumberOrNull(overview?.bank?.cashBalance);
    const careerLevel = asNumberOrNull(overview?.reputation?.level);
    const careerXp = asNumberOrNull(overview?.reputation?.xpPoints);
    const fleetTrucks = asNumberOrNull(overview?.fleetOverview?.trucks);
    const fleetTrailers = asNumberOrNull(overview?.fleetOverview?.trailers);
    const careerFleet =
      fleetTrucks !== null && fleetTrailers !== null
        ? `${Math.max(0, Math.floor(fleetTrucks))} / ${Math.max(0, Math.floor(fleetTrailers))}`
        : emptyMetric;

    const repLabel = overview?.reputation?.label ?? null;
    const careerReputation = repLabel && careerLevel !== null
      ? `${repLabel} / L${careerLevel}`
      : careerLevel !== null
        ? `L${careerLevel}`
        : emptyMetric;
    const dashboard = overview?.dashboard;
    const employeeOverview = overview?.employeeOverview;
    const jobStats = overview?.jobStats;

    if (refs.editorProfileValue) refs.editorProfileValue.textContent = profileLabel;
    if (refs.editorSaveValue) refs.editorSaveValue.textContent = saveLabel;
    if (refs.editorMoneyValue) refs.editorMoneyValue.textContent = formatCurrency(money);
    if (refs.editorXpValue) refs.editorXpValue.textContent = formatTelemetryNumber(xp, 0);
    if (refs.editorLevelValue) refs.editorLevelValue.textContent = String(level);
    if (refs.editorFleetValue) refs.editorFleetValue.textContent = `${truckCount} / ${trailerCount}`;

    if (refs.careerCompanyValue) refs.careerCompanyValue.textContent = companyLabel;
    if (refs.careerSidebarCompany) refs.careerSidebarCompany.textContent = companyLabel;
    setTextOrEmpty(refs.careerSidebarBalance, cashBalance, formatCurrency);
    setTextOrEmpty(refs.careerBalanceValue, cashBalance, formatCurrency);
    if (refs.careerReputationValue) refs.careerReputationValue.textContent = careerReputation;
    if (refs.careerFleetStatusValue) refs.careerFleetStatusValue.textContent = careerFleet;

    if (refs.careerProfileName) refs.careerProfileName.textContent = profileLabel;
    if (refs.careerCompanyHeadline) refs.careerCompanyHeadline.textContent = companyLabel;

    if (refs.careerProfileAvatar) {
      const src = document.getElementById("activeProfileIcon")?.getAttribute("src") || getThemeFallbackIcon();
      if (refs.careerProfileAvatar.getAttribute("src") !== src) {
        refs.careerProfileAvatar.setAttribute("src", src);
      }
      refs.careerProfileAvatar.onerror = () => handleIconError(refs.careerProfileAvatar);
    }

    const levelSpan = 1500;
    const xpIntoLevel =
      careerXp === null
        ? 0
        : ((Number(careerXp) % levelSpan) + levelSpan) % levelSpan;
    const progress =
      careerXp === null || levelSpan <= 0
        ? 0
        : Math.max(0, Math.min(xpIntoLevel / levelSpan, 1));

    if (refs.careerLevelValue) refs.careerLevelValue.textContent = careerLevel === null ? emptyMetric : `L${careerLevel}`;
    if (refs.careerXpValue) refs.careerXpValue.textContent = careerXp === null ? emptyMetric : formatTelemetryNumber(careerXp, 0);
    if (refs.careerLevelProgressFill) refs.careerLevelProgressFill.style.width = `${Math.round(progress * 100)}%`;
    if (refs.careerLevelProgressText) {
      refs.careerLevelProgressText.textContent = careerXp === null
        ? emptyMetric
        : `${Math.round(progress * 100)}% • ${Math.round(xpIntoLevel)} / ${levelSpan}`;
    }

    setTextOrEmpty(refs.careerJobsTotalValue, asNumberOrNull(jobStats?.totalJobs), (v) => String(Math.max(0, Math.floor(v))));
    setTextOrEmpty(refs.careerJobsTotalIncomeValue, asNumberOrNull(jobStats?.totalIncome), formatCurrency);
    setTextOrEmpty(refs.careerJobsAverageDistanceValue, asNumberOrNull(jobStats?.averageDistanceKm), formatDistance);
    if (refs.careerJobsSuccessRateValue) {
      const rate = asNumberOrNull(jobStats?.successRate);
      refs.careerJobsSuccessRateValue.textContent = rate === null ? emptyMetric : `${Math.round(rate * 100)}%`;
    }

    setTextOrEmpty(refs.careerLiveRevenueValue, asNumberOrNull(dashboard?.liveIncome), formatCurrency);
    setTextOrEmpty(refs.careerCostFuelValue, asNumberOrNull(dashboard?.fuelCost), formatCurrency);
    setTextOrEmpty(refs.careerCostRepairValue, asNumberOrNull(dashboard?.repairCost), formatCurrency);
    setTextOrEmpty(refs.careerCostTollValue, asNumberOrNull(dashboard?.tollCost), formatCurrency);
    if (refs.careerDriversOnlineValue) refs.careerDriversOnlineValue.textContent = formatTwoDigitOrEmpty(employeeOverview?.onDuty);
    if (refs.careerDriversRestingValue) refs.careerDriversRestingValue.textContent = formatTwoDigitOrEmpty(employeeOverview?.resting);
    const activeTrip = overview?.activeTrip;
    if (refs.careerActiveTripValue) {
      refs.careerActiveTripValue.textContent = activeTrip
        ? `${activeTrip.origin || "--"} -> ${activeTrip.destination || "--"}`
        : careerUi.noActiveTrip;
    }
    if (refs.careerActiveTripSummary) {
      refs.careerActiveTripSummary.textContent = activeTrip
        ? `${formatDistance(activeTrip.distanceKm || 0)} | ${formatDurationCompact(activeTrip.durationSeconds || 0)}`
        : careerUi.tripWaiting;
    }

    renderActiveJobCard();
    renderRecentJobs();
    renderCareerActivityFeed();
    renderCareerLogbook();
  };

  const renderCareerOverview = async (overview) => {
    careerOverview = overview;
    clearCareerLoadFailure();
    updateOperationalOverview();
    if (overview?.lastTelemetry) {
      renderTelemetry(overview.lastTelemetry);
    }
    if (activeCareerPanel !== "dashboard") {
      await showCareerPanel(activeCareerPanel);
      return;
    }
    applyCareerState();
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
    const speed = Number(data?.speed ?? data?.speed_kph ?? 0);
    const fuel = Number(data?.fuel ?? data?.fuel_liters ?? 0);
    const capacity = Number(data?.fuelCapacity ?? data?.fuel_capacity_liters ?? 0);
    const ratio = capacity > 0 ? Math.max(0, Math.min(fuel / capacity, 1)) : 0;
    careerState.pluginInstalled =
      typeof data?.pluginInstalled === "boolean" ? data.pluginInstalled : careerState.pluginInstalled;
    careerState.bridgeConnected =
      typeof data?.sdkConnected === "boolean" ? data.sdkConnected : careerState.bridgeConnected;
    careerState.paused = Number(data?.paused ?? 0) === 1;
    careerState.engineOn = Boolean(data?.engineOn ?? careerState.engineOn);
    if (refs.careerSpeedDial) refs.careerSpeedDial.style.setProperty("--dial-progress", String(Math.min(speed / 180, 1)));
    if (refs.careerSpeedValue) refs.careerSpeedValue.textContent = String(Math.round(speed));
    if (refs.careerGearValue) {
      if (typeof data?.gear === "string") {
        refs.careerGearValue.textContent = data.gear;
      } else {
        const gear = Number(data?.gear ?? 0);
        refs.careerGearValue.textContent = gear === 0 ? "N" : gear > 0 ? String(gear) : `R${Math.abs(gear)}`;
      }
    }
    if (refs.careerFuelValue) {
      refs.careerFuelValue.textContent =
        capacity > 0
          ? `${formatTelemetryNumber(fuel, 1)} / ${formatTelemetryNumber(capacity, 1)} L`
          : `${formatTelemetryNumber(fuel, 1)} L`;
    }
    if (refs.careerFuelPercent) refs.careerFuelPercent.textContent = `${Math.round(ratio * 100)}%`;
    if (refs.careerFuelBarFill) refs.careerFuelBarFill.style.setProperty("--fuel-progress", String(ratio));
    if (refs.careerRpmValue) refs.careerRpmValue.textContent = formatTelemetryNumber(data?.rpm ?? data?.engine_rpm ?? 0, 0);
    applyCareerState();
  };

  let lastTelemetryRenderAt = 0;
  let lastVehicleTelemetryAt = 0;
  let telemetryRenderTimer = 0;
  let pendingTelemetryPayload = null;

  const scheduleTelemetryRender = (payload) => {
    pendingTelemetryPayload = payload;
    if (telemetryRenderTimer) return;
    const now = Date.now();
    const waitMs = Math.max(0, 100 - (now - lastTelemetryRenderAt));
    telemetryRenderTimer = window.setTimeout(() => {
      telemetryRenderTimer = 0;
      lastTelemetryRenderAt = Date.now();
      renderTelemetry(pendingTelemetryPayload);
    }, waitMs);
  };

  const renderCareerStatus = (status) => {
    clearCareerLoadFailure();
    careerState.gameRunning = Boolean(status?.ets2_running || status?.ats_running);
    careerState.pluginInstalled = Boolean(status?.plugin_installed);
    careerState.bridgeConnected = Boolean(status?.bridge_connected);
    careerState.activeGame = status?.active_game || careerState.activeGame || lastSelectedGame || "ets2";
    if (!careerState.bridgeConnected) {
      careerState.paused = false;
      careerState.engineOn = false;
    }
    applyCareerState();
  };
  const updateCareerFlag = (key, value) => {
    careerState[key] = Boolean(value);
    if (key === "bridgeConnected" && !careerState.bridgeConnected) {
      careerState.paused = false;
      careerState.engineOn = false;
    }
    if (key === "pluginInstalled" && !careerState.pluginInstalled) {
      careerState.engineOn = false;
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

  const resolveAuthErrorMessage = async (error) => {
    const raw = String(error?.message || error || "").replace(/^Error:\s*/, "").trim();
    if (!raw) return await t("career.auth.errors.unknown");
    const map = {
      "Email is required": "career.auth.errors.email_required",
      "Email is invalid": "career.auth.errors.email_invalid",
      "Email not found": "career.auth.errors.email_not_found",
      "Email is already registered": "career.auth.errors.email_taken",
      "Username is required": "career.auth.errors.username_required",
      "Username is too short": "career.auth.errors.username_too_short",
      "Username is already taken": "career.auth.errors.username_taken",
      "Password must be at least 8 characters": "career.auth.errors.password_too_short",
      "Password confirmation does not match": "career.auth.errors.password_confirm_mismatch",
      "Invalid password": "career.auth.errors.password_invalid",
      "Invalid recovery code": "career.auth.errors.recovery_code_invalid",
      "Consent is required": "career.auth.errors.consent_required",
      "Not authenticated": "career.auth.errors.not_authenticated",
      "Forbidden": "career.auth.errors.forbidden",
    };
    if (raw in map) return await t(map[raw]);
    return raw;
  };

  refs.careerAuthLoginTab?.addEventListener("click", () => applyAuthTab("login"));
  refs.careerAuthRegisterTab?.addEventListener("click", () => applyAuthTab("register"));

  refs.careerLoginSubmit?.addEventListener("click", async () => {
    setInlineError(refs.careerLoginError, "");
    const email = refs.careerLoginEmail?.value?.trim() || "";
    const password = refs.careerLoginPassword?.value || "";

    if (!email) {
      setInlineError(refs.careerLoginError, await t("career.auth.errors.email_required"));
      return;
    }
    if (!password) {
      setInlineError(refs.careerLoginError, await t("career.auth.errors.password_required"));
      return;
    }

    refs.careerLoginSubmit.disabled = true;
    setCareerAuthStatus(await t("career.auth.status_logging_in"));
    try {
      await invoke("auth_login", { email, password, rememberMe: true });
      setCareerAuthStatus("");
      invalidPasswordAttempts = 0;
      if (refs.careerForgotPasswordGate) refs.careerForgotPasswordGate.hidden = true;
      await refreshCareerOnboardingGate();
    } catch (error) {
      setCareerAuthStatus("");
      setInlineError(refs.careerLoginError, await resolveAuthErrorMessage(error));
      const raw = String(error?.message || error || "").replace(/^Error:\s*/, "").trim();
      if (raw === "Invalid password") {
        invalidPasswordAttempts += 1;
        if (refs.careerForgotPasswordGate) refs.careerForgotPasswordGate.hidden = invalidPasswordAttempts < 3;
      }
      console.error("[career] login failed", error);
    } finally {
      refs.careerLoginSubmit.disabled = false;
    }
  });

  refs.careerLoginCancel?.addEventListener("click", () => {
    invalidPasswordAttempts = 0;
    if (refs.careerForgotPasswordGate) refs.careerForgotPasswordGate.hidden = true;
    setCareerAuthStatus("");
    hideCareerAuthGate();
    void activateMode("editor");
    setHubVisibility(true);
  });

  refs.userMenuLogin?.addEventListener("click", async () => {
    setUserMenuOpen(false);
    invalidPasswordAttempts = 0;
    if (refs.careerForgotPasswordGate) refs.careerForgotPasswordGate.hidden = true;
    await activateMode("career");
    showCareerLoginView();
    setCareerAuthStatus("");
  });

  refs.userMenuLogout?.addEventListener("click", async () => {
    setUserMenuOpen(false);
    try {
      await invoke("auth_logout");
    } catch (error) {
      console.error("[auth] logout failed", error);
    }
    pendingRecoveryCodes = null;
    window.careerAuthUser = null;
    void updateUserMenu();
    void refreshCareerOnboardingGate();
  });

  refs.userMenuAdmin?.addEventListener("click", async () => {
    setUserMenuOpen(false);
    await activateMode("career");
    await showCareerPanel("admin_db");
  });

  refs.careerForgotPasswordOpen?.addEventListener("click", () => showPasswordResetPanel());
  refs.careerResetCancel?.addEventListener("click", () => showLoginPanel());

  refs.careerResetSubmit?.addEventListener("click", async () => {
    setInlineError(refs.careerResetError, "");
    const email = refs.careerResetEmail?.value?.trim() || "";
    const recoveryCode = refs.careerResetRecoveryCode?.value?.trim() || "";
    const newPassword = refs.careerResetNewPassword?.value || "";
    const newPasswordConfirm = refs.careerResetPasswordConfirm?.value || "";

    if (!email) {
      setInlineError(refs.careerResetError, await t("career.auth.errors.email_required"));
      return;
    }
    if (!recoveryCode) {
      setInlineError(refs.careerResetError, await t("career.auth.errors.recovery_code_required"));
      return;
    }
    if (!newPassword) {
      setInlineError(refs.careerResetError, await t("career.auth.errors.password_required"));
      return;
    }
    if (!newPasswordConfirm) {
      setInlineError(refs.careerResetError, await t("career.auth.errors.password_confirm_required"));
      return;
    }

    refs.careerResetSubmit.disabled = true;
    setCareerAuthStatus(await t("career.auth.status_resetting"));
    try {
      await invoke("auth_reset_password_with_recovery_code", {
        email,
        recoveryCode,
        newPassword,
        newPasswordConfirm,
      });
      setCareerAuthStatus("");
      window.showToast("career.auth.reset_success", "success");
      showLoginPanel();
    } catch (error) {
      setCareerAuthStatus("");
      setInlineError(refs.careerResetError, await resolveAuthErrorMessage(error));
      console.error("[career] password reset failed", error);
    } finally {
      refs.careerResetSubmit.disabled = false;
    }
  });

  refs.careerRegisterSubmit?.addEventListener("click", async () => {
    setInlineError(refs.careerRegisterError, "");
    const username = refs.careerRegisterUsername?.value?.trim() || "";
    const email = refs.careerRegisterEmail?.value?.trim() || "";
    const password = refs.careerRegisterPassword?.value || "";
    const passwordConfirm = refs.careerRegisterPasswordConfirm?.value || "";
    const consentPrivacy = Boolean(refs.careerRegisterConsentPrivacy?.checked);
    const consentTerms = Boolean(refs.careerRegisterConsentTerms?.checked);

    if (!username) {
      setInlineError(refs.careerRegisterError, await t("career.auth.errors.username_required"));
      return;
    }
    if (!email) {
      setInlineError(refs.careerRegisterError, await t("career.auth.errors.email_required"));
      return;
    }
    if (!password) {
      setInlineError(refs.careerRegisterError, await t("career.auth.errors.password_required"));
      return;
    }
    if (!passwordConfirm) {
      setInlineError(refs.careerRegisterError, await t("career.auth.errors.password_confirm_required"));
      return;
    }
    if (!(consentPrivacy && consentTerms)) {
      setInlineError(refs.careerRegisterError, await t("career.auth.errors.consent_required"));
      return;
    }

    refs.careerRegisterSubmit.disabled = true;
    setCareerAuthStatus(await t("career.auth.status_registering"));
    try {
      await invoke("auth_register", {
        username,
        email,
        password,
        passwordConfirm,
        consentPrivacy,
        consentTerms,
        rememberMe: true,
      });
      setCareerAuthStatus("");
      await refreshCareerOnboardingGate();
    } catch (error) {
      setCareerAuthStatus("");
      setInlineError(refs.careerRegisterError, await resolveAuthErrorMessage(error));
      console.error("[career] register failed", error);
    } finally {
      refs.careerRegisterSubmit.disabled = false;
    }
  });

  const onboardingTabHandler = (tab) => {
    applyOnboardingTab(tab);
    if (tab === "join") {
      renderCompanyList(cachedCompanyList, refs.careerCompanySearch?.value || "");
    }
  };

  refs.careerOnboardingJoinTab?.addEventListener("click", () => onboardingTabHandler("join"));
  refs.careerOnboardingCreateTab?.addEventListener("click", () => onboardingTabHandler("create"));

  refs.careerCompanySearch?.addEventListener("input", () => {
    renderCompanyList(cachedCompanyList, refs.careerCompanySearch?.value || "");
  });

  refs.careerCompanyList?.addEventListener("click", async (event) => {
    const button = event.target.closest("[data-career-company-join]");
    if (!button) return;
    const id = Number(button.getAttribute("data-career-company-join") || 0);
    if (!id) return;

    // MVP: Direct join without invitations / approvals.
    button.disabled = true;
    setCareerAuthStatus(await t("career.onboarding.status_joining"));
    try {
      await invoke("company_join", { companyId: id });
      cachedCompanyListAt = 0;
      setCareerAuthStatus("");
      await refreshCareerOnboardingGate();
    } catch (error) {
      setCareerAuthStatus("");
      window.showToast(await resolveAuthErrorMessage(error), "error");
      console.error("[career] company join failed", error);
    } finally {
      button.disabled = false;
    }
  });

  refs.careerCompanyCreateSubmit?.addEventListener("click", async () => {
    setInlineError(refs.careerCompanyCreateError, "");

    const name = refs.careerCompanyName?.value?.trim() || "";
    const location = refs.careerCompanyLocation?.value?.trim() || "";
    const language = refs.careerCompanyLanguage?.value?.trim() || "";
    const game = refs.careerCompanyGame?.value?.trim() || "";
    const descriptionRaw = refs.careerCompanyDescription?.value || "";
    const description = descriptionRaw.trim() ? descriptionRaw.trim().slice(0, 500) : null;

    if (!name) {
      setInlineError(refs.careerCompanyCreateError, await t("career.company.errors.name_required"));
      return;
    }
    if (!location) {
      setInlineError(refs.careerCompanyCreateError, await t("career.company.errors.location_required"));
      return;
    }
    if (!language) {
      setInlineError(refs.careerCompanyCreateError, await t("career.company.errors.language_required"));
      return;
    }
    if (!game) {
      setInlineError(refs.careerCompanyCreateError, await t("career.company.errors.game_required"));
      return;
    }

    refs.careerCompanyCreateSubmit.disabled = true;
    setCareerAuthStatus(await t("career.onboarding.status_creating"));
    try {
      const logo = await readFileBase64(refs.careerCompanyLogo);
      const header = await readFileBase64(refs.careerCompanyHeader);

      await invoke("company_create_onboarding", {
        name,
        location,
        language,
        game,
        description,
        logoBase64: logo.base64,
        logoMime: logo.mime,
        headerBase64: header.base64,
        headerMime: header.mime,
      });
      cachedCompanyListAt = 0;
      setCareerAuthStatus("");
      await refreshCareerOnboardingGate();
    } catch (error) {
      setCareerAuthStatus("");
      setInlineError(refs.careerCompanyCreateError, await resolveAuthErrorMessage(error));
      console.error("[career] company create failed", error);
    } finally {
      refs.careerCompanyCreateSubmit.disabled = false;
    }
  });

  refs.careerDetailHost?.addEventListener("click", async (event) => {
    const actionButton = event.target.closest("[data-career-action]");
    if (!actionButton) return;

    const { careerAction, jobId } = actionButton.dataset;
    if (actionButton.disabled) return;

    if (careerAction === "generate-recovery-codes") {
      actionButton.disabled = true;
      try {
        const codes = await invoke("auth_generate_recovery_codes");
        pendingRecoveryCodes = Array.isArray(codes) ? codes : null;
        window.showToast("career.account.recovery_codes.generated_toast", "success");
        await refreshCareerPanel();
      } catch (error) {
        console.error("[career] recovery code generation failed", error);
        window.showToast("career.account.recovery_codes.generate_failed", "error");
      } finally {
        actionButton.disabled = false;
      }
      return;
    }

    if (careerAction === "generate-jobs") {
      actionButton.disabled = true;
      try {
        await invoke("career_generate_jobs");
        await renderCareerOverview(await invoke("career_get_overview"));
      } catch (error) {
        console.error("[career] job generation failed", error);
        window.showToast("career.jobs.refresh_failed", "error");
      } finally {
        actionButton.disabled = false;
      }
      return;
    }

    if (careerAction === "accept-job" && jobId) {
      actionButton.disabled = true;
      try {
        await invoke("career_accept_job", { jobId });
        await renderCareerOverview(await invoke("career_get_overview"));
        window.showToast("career.jobs.accept_success", "success");
      } catch (error) {
        console.error("[career] job accept failed", error);
        window.showToast("career.jobs.accept_failed", "error");
      } finally {
        actionButton.disabled = false;
      }
    }
  });

  refs.editorModeBtn?.addEventListener("click", () => activateMode("editor"));
  refs.careerModeBtn?.addEventListener("click", () => activateMode("career"));
  refs.hubEditorCard?.addEventListener("click", () => activateMode("editor"));
  refs.hubCareerCard?.addEventListener("click", () => activateMode("career"));
  refs.hubHomeBtn?.addEventListener("click", () => setHubVisibility(true));
  refs.saveSafeModeBtn?.addEventListener("click", () => setEditorPresentationMode("safe"));
  refs.saveAdvancedModeBtn?.addEventListener("click", () => setEditorPresentationMode("advanced"));
  let pendingCareerOverview = null;
  let careerOverviewRenderScheduled = false;

  const scheduleCareerOverviewRender = (overview) => {
    pendingCareerOverview = overview;
    if (careerOverviewRenderScheduled) return;
    careerOverviewRenderScheduled = true;
    window.requestAnimationFrame(() => {
      careerOverviewRenderScheduled = false;
      try {
        void renderCareerOverview(pendingCareerOverview);
      } catch (error) {
        console.warn("[career] overview render failed", error);
      }
    });
  };

  if (!window.__ets2_tool_listeners_registered) {
    window.__ets2_tool_listeners_registered = true;

    listen("hub://mode_changed", (event) => {
      if (isStandaloneEditorPage) return;
      applyHubMode(event.payload.mode ?? event.payload);
    }).catch(console.error);
    listen("telemetry:update", (event) => {
      lastVehicleTelemetryAt = Date.now();
      scheduleTelemetryRender(event.payload);
    }).catch(console.error);
    listen("career://game_running", (event) => updateCareerFlag("gameRunning", event.payload)).catch(console.error);
    listen("career://plugin_installed", (event) => updateCareerFlag("pluginInstalled", event.payload)).catch(console.error);
    listen("career://bridge_connected", (event) => updateCareerFlag("bridgeConnected", event.payload)).catch(console.error);
    listen("career://status", (event) => renderCareerStatus(event.payload)).catch(console.error);
    listen("career://overview", (event) => {
      try {
        if (localStorage.getItem("career_job_debug") === "1") {
          console.log("[career_job_debug] overview.activeJob:", event.payload?.activeJob);
          console.log("[career_job_debug] overview.recentJobs:", event.payload?.recentJobs);
          console.log("[career_job_debug] overview.jobStats:", event.payload?.jobStats);
        }
      } catch (error) {
        console.warn("[career_job_debug] logging failed", error);
      }
      scheduleCareerOverviewRender(event.payload);
    }).catch(console.error);
    listen("career://telemetry_tick", (event) => {
      if (Date.now() - lastVehicleTelemetryAt < 1000) return;
      scheduleTelemetryRender(event.payload);
    }).catch(console.error);
  }

  if (hasTauri) {
    const initialMode = isStandaloneEditorPage
      ? "editor"
      : await safeInvoke("hub_get_mode", {}, { fallback: "editor", silent: true });
    console.log("[ui] initial mode", initialMode);
    applyHubMode(initialMode || "editor");
    setHubVisibility(false);

    try {
      await invoke("auth_restore_session");
    } catch (error) {
      console.warn("[auth] restore session failed", error);
    }

    try {
      window.careerAuthUser = await invoke("auth_get_current_user");
    } catch {
      window.careerAuthUser = null;
    }
    await updateUserMenu();
  } else {
    applyHubMode("editor");
    setHubVisibility(true);
    window.careerAuthUser = null;
    await updateUserMenu();
  }

  try {
    const selectedGame = await invoke("get_selected_game");
    careerState.activeGame = selectedGame;
    setCareerGame(selectedGame);
    lastSelectedGame = selectedGame;
    console.log("[ui] selected game", selectedGame);
  } catch (error) {
    console.warn("[ui] selected game load failed", error);
  }

  try {
    renderCareerStatus(await invoke("career_get_status"));
    console.log("[ui] career status loaded");
  } catch (error) {
    console.warn("[ui] career status load failed", error);
  }

  try {
    await renderCareerOverview(await invoke("career_get_overview"));
    console.log("[ui] career overview loaded");
  } catch (error) {
    console.warn("[ui] career overview load failed", error);
  }

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
    if (!event.target.closest(".user-menu")) setUserMenuOpen(false);
  });

  refs.userMenuBtn?.addEventListener("click", (event) => {
    event.stopPropagation();
    toggleUserMenu();
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") setUserMenuOpen(false);
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
  clearCareerLoadFailure();
  console.log("[ui] boot complete");
  } catch (error) {
    console.error("[ui] boot failed", error);
    showCareerLoadFailure(error);
  }
});
