const COPY_KEYS = {
  title: "garage_manager.title",
  description: "garage_manager.description",
  refresh: "garage_manager.actions.refresh",
  details: "garage_manager.actions.details",
  edit: "garage_manager.actions.edit",
  purchase: "garage_manager.actions.purchase",
  purchaseAll: "garage_manager.actions.purchase_all",
  relinquishEmpty: "garage_manager.actions.relinquish_empty",
  relinquish: "garage_manager.actions.relinquish",
  assignResources: "garage_manager.actions.assign_resources",
  assignRandomDrivers: "garage_manager.actions.assign_drivers",
  assignDriver: "garage_manager.actions.assign_driver",
  manageDrivers: "garage_manager.actions.manage_drivers",
  done: "garage_manager.actions.done",
  upgrade: "garage_manager.actions.upgrade",
  downgrade: "garage_manager.actions.downgrade",
  setHeadquarters: "garage_manager.actions.set_headquarters",
  retry: "garage_manager.actions.retry",
  resetFilters: "garage_manager.actions.reset_filters",
  confirm: "garage_manager.actions.confirm",
  cancel: "garage_manager.actions.cancel",
  close: "garage_manager.actions.close",
  apply: "garage_manager.actions.apply",
  refreshing: "garage_manager.actions.refreshing",
  purchasing: "garage_manager.actions.purchasing",
  purchasingAll: "garage_manager.actions.purchasing_all",
  relinquishingEmpty: "garage_manager.actions.relinquishing_empty",
  relinquishing: "garage_manager.actions.relinquishing",
  assigningResources: "garage_manager.actions.assigning_resources",
  assigningDrivers: "garage_manager.actions.assigning_drivers",
  upgrading: "garage_manager.actions.upgrading",
  downgrading: "garage_manager.actions.downgrading",
  changingHeadquarters: "garage_manager.actions.changing_headquarters",
  citySearchLabel: "garage_manager.filters.city_search_label",
  citySearchPlaceholder: "garage_manager.filters.city_search_placeholder",
  idSearchLabel: "garage_manager.filters.id_search_label",
  idSearchPlaceholder: "garage_manager.filters.id_search_placeholder",
  searchLabel: "garage_manager.filters.search_label",
  searchPlaceholder: "garage_manager.filters.search_placeholder",
  ownershipLabel: "garage_manager.filters.ownership_label",
  ownershipAll: "garage_manager.filters.ownership_all",
  ownershipOwned: "garage_manager.filters.ownership_owned",
  ownershipNotOwned: "garage_manager.filters.ownership_not_owned",
  sizeLabelFilter: "garage_manager.filters.size_label",
  sizeAll: "garage_manager.filters.size_all",
  sizeUnownedFilter: "garage_manager.filters.size_unowned",
  sizeSmallFilter: "garage_manager.filters.size_small",
  sizeLargeFilter: "garage_manager.filters.size_large",
  hqLabel: "garage_manager.filters.hq_label",
  hqAll: "garage_manager.filters.hq_all",
  hqOnly: "garage_manager.filters.hq_only",
  hqExclude: "garage_manager.filters.hq_exclude",
  occupancyLabel: "garage_manager.filters.occupancy_label",
  occupancyAll: "garage_manager.filters.occupancy_all",
  occupancyFree: "garage_manager.filters.occupancy_free",
  occupancyFull: "garage_manager.filters.occupancy_full",
  sortLabel: "garage_manager.filters.sort_label",
  sortCity: "garage_manager.filters.sort_city",
  sortSize: "garage_manager.filters.sort_size",
  sortFree: "garage_manager.filters.sort_free",
  sortDrivers: "garage_manager.filters.sort_drivers",
  sortTrucks: "garage_manager.filters.sort_trucks",
  activeFilterOne: "garage_manager.filters.active_one",
  activeFilterMany: "garage_manager.filters.active_many",
  total: "garage_manager.summary.total",
  owned: "garage_manager.summary.owned",
  notOwned: "garage_manager.summary.not_owned",
  smallGarages: "garage_manager.summary.small",
  largeGarages: "garage_manager.summary.large",
  freeSlots: "garage_manager.summary.free_slots",
  occupiedSlotsSummary: "garage_manager.summary.occupied_slots",
  drivers: "garage_manager.summary.drivers",
  trucks: "garage_manager.summary.trucks",
  trailers: "garage_manager.summary.trailers",
  hq: "garage_manager.status.hq",
  regular: "garage_manager.status.regular",
  ownershipOwnedValue: "garage_manager.status.owned",
  ownershipNotOwnedValue: "garage_manager.status.not_owned",
  ownershipUnknown: "garage_manager.status.ownership_unknown",
  warningStatus: "garage_manager.status.warning",
  writeReady: "garage_manager.status.write_ready",
  writeBlocked: "garage_manager.status.write_blocked",
  writeReadOnly: "garage_manager.status.write_read_only",
  sizeUnowned: "garage_manager.size.unowned",
  sizeSmall: "garage_manager.size.small",
  sizeLarge: "garage_manager.size.large",
  sizeUnknown: "garage_manager.size.unknown",
  slotCount: "garage_manager.size.slot_count",
  occupancyValue: "garage_manager.size.occupancy_value",
  cityId: "garage_manager.fields.city_id",
  garageId: "garage_manager.fields.garage_id",
  currentSize: "garage_manager.fields.current_size",
  maximumSize: "garage_manager.fields.maximum_size",
  occupiedSlots: "garage_manager.fields.occupied_slots",
  availableSlots: "garage_manager.fields.available_slots",
  assignedDrivers: "garage_manager.fields.assigned_drivers",
  freeDriverPositions: "garage_manager.fields.free_driver_positions",
  availableAiDrivers: "garage_manager.fields.available_ai_drivers",
  aiDriverPool: "garage_manager.fields.ai_driver_pool",
  remainingDriverPool: "garage_manager.fields.remaining_driver_pool",
  assignedTrucks: "garage_manager.fields.assigned_trucks",
  assignedTrailers: "garage_manager.fields.assigned_trailers",
  productivity: "garage_manager.fields.productivity",
  warnings: "garage_manager.fields.warnings",
  status: "garage_manager.fields.status",
  capacity: "garage_manager.fields.capacity",
  headquarters: "garage_manager.fields.headquarters",
  writeStatus: "garage_manager.fields.write_status",
  backupStatus: "garage_manager.fields.backup_status",
  verificationStatus: "garage_manager.fields.verification_status",
  command: "garage_manager.fields.command",
  errorCode: "garage_manager.fields.error_code",
  backendMessage: "garage_manager.fields.backend_message",
  backendStatus: "garage_manager.fields.backend_status",
  none: "garage_manager.values.none",
  unknown: "garage_manager.values.unknown",
  notAvailable: "garage_manager.values.not_available",
  loading: "garage_manager.states.loading",
  noProfile: "garage_manager.states.no_profile",
  noSave: "garage_manager.states.no_save",
  noGarages: "garage_manager.states.no_garages",
  noResults: "garage_manager.states.no_results",
  loadError: "garage_manager.states.load_error",
  atsReadOnly: "garage_manager.states.ats_read_only",
  mutationBlocked: "garage_manager.states.mutation_blocked",
  loadingHint: "garage_manager.states.loading_hint",
  refreshError: "garage_manager.states.refresh_error",
  pathHidden: "garage_manager.states.path_hidden",
  profileLabel: "garage_manager.context.profile",
  saveLabel: "garage_manager.context.save",
  noProfileShort: "garage_manager.context.no_profile",
  noSaveShort: "garage_manager.context.no_save",
  detailTitle: "garage_manager.details.title",
  generalInformation: "garage_manager.details.general_information",
  assignments: "garage_manager.details.assignments",
  availableActions: "garage_manager.details.available_actions",
  slotAssignments: "garage_manager.details.slot_assignments",
  driverReferences: "garage_manager.details.driver_references",
  truckReferences: "garage_manager.details.truck_references",
  trailerReferences: "garage_manager.details.trailer_references",
  driversCount: "garage_manager.details.drivers_count",
  trucksCount: "garage_manager.details.trucks_count",
  trailersCount: "garage_manager.details.trailers_count",
  noDrivers: "garage_manager.details.no_drivers",
  noTrucks: "garage_manager.details.no_trucks",
  noTrailers: "garage_manager.details.no_trailers",
  noWarnings: "garage_manager.details.no_warnings",
  editTitle: "garage_manager.edit.title",
  editSize: "garage_manager.edit.size",
  editSetHq: "garage_manager.edit.set_hq",
  editHqCurrent: "garage_manager.edit.hq_current",
  noChanges: "garage_manager.edit.no_changes",
  confirmTitle: "garage_manager.confirm.title",
  affectedCity: "garage_manager.confirm.affected_city",
  desiredChange: "garage_manager.confirm.desired_change",
  cost: "garage_manager.confirm.cost",
  costNone: "garage_manager.confirm.cost_none",
  backup: "garage_manager.confirm.backup",
  backupAutomatic: "garage_manager.confirm.backup_automatic",
  saveWarning: "garage_manager.confirm.save_warning",
  purchaseChange: "garage_manager.confirm.purchase_change",
  purchaseAllChange: "garage_manager.confirm.purchase_all_change",
  relinquishEmptyChange: "garage_manager.confirm.relinquish_empty_change",
  relinquishEmptyEffect: "garage_manager.confirm.relinquish_empty_effect",
  relinquishChange: "garage_manager.confirm.relinquish_change",
  relinquishEffect: "garage_manager.confirm.relinquish_effect",
  assignResourcesChange: "garage_manager.confirm.assign_resources_change",
  assignDriversChange: "garage_manager.confirm.assign_drivers_change",
  assignResourcesEffect: "garage_manager.confirm.assign_resources_effect",
  assignDriversEffect: "garage_manager.confirm.assign_drivers_effect",
  upgradeChange: "garage_manager.confirm.upgrade_change",
  updateLargeChange: "garage_manager.confirm.update_large_change",
  updateSmallChange: "garage_manager.confirm.update_small_change",
  updateHqChange: "garage_manager.confirm.update_hq_change",
  currentOccupancy: "garage_manager.confirm.current_occupancy",
  newCapacity: "garage_manager.confirm.new_capacity",
  downgradeWarning: "garage_manager.confirm.downgrade_warning",
  purchaseNoExtras: "garage_manager.confirm.purchase_no_extras",
  applying: "garage_manager.confirm.applying",
  technicalDetails: "garage_manager.errors.technical_details",
  saveChangedHint: "garage_manager.errors.save_changed_hint",
  verificationNotCompleted: "garage_manager.errors.verification_not_completed",
  loadFailureTitle: "garage_manager.errors.load_failure_title",
  purchaseFailureTitle: "garage_manager.errors.purchase_failure_title",
  purchaseAllFailureTitle: "garage_manager.errors.purchase_all_failure_title",
  relinquishEmptyFailureTitle: "garage_manager.errors.relinquish_empty_failure_title",
  relinquishFailureTitle: "garage_manager.errors.relinquish_failure_title",
  assignResourcesFailureTitle: "garage_manager.errors.assign_resources_failure_title",
  assignDriversFailureTitle: "garage_manager.errors.assign_drivers_failure_title",
  garageAssignmentDriverCountInvalid: "garage_manager.errors.garage_assignment_driver_count_invalid",
  upgradeFailureTitle: "garage_manager.errors.upgrade_failure_title",
  downgradeFailureTitle: "garage_manager.errors.downgrade_failure_title",
  headquartersFailureTitle: "garage_manager.errors.headquarters_failure_title",
  purchaseDialogTitle: "garage_manager.dialogs.purchase_title",
  purchaseAllDialogTitle: "garage_manager.dialogs.purchase_all_title",
  relinquishEmptyDialogTitle: "garage_manager.dialogs.relinquish_empty_title",
  relinquishDialogTitle: "garage_manager.dialogs.relinquish_title",
  assignResourcesDialogTitle: "garage_manager.dialogs.assign_resources_title",
  upgradeDialogTitle: "garage_manager.dialogs.upgrade_title",
  downgradeDialogTitle: "garage_manager.dialogs.downgrade_title",
  headquartersDialogTitle: "garage_manager.dialogs.headquarters_title",
  currentState: "garage_manager.dialogs.current_state",
  futureState: "garage_manager.dialogs.future_state",
  effects: "garage_manager.dialogs.effects",
  currentHeadquarters: "garage_manager.dialogs.current_headquarters",
  newHeadquarters: "garage_manager.dialogs.new_headquarters",
  downgradeBlocked: "garage_manager.dialogs.downgrade_blocked",
  relinquishBlocked: "garage_manager.dialogs.relinquish_blocked",
  optionalExtensions: "garage_manager.dialogs.optional_extensions",
  addTrucks: "garage_manager.dialogs.add_trucks",
  assignDrivers: "garage_manager.dialogs.assign_drivers",
  optionDisabledDefault: "garage_manager.dialogs.option_disabled_default",
  optionUnavailable: "garage_manager.dialogs.option_unavailable",
  randomDriver: "garage_manager.dialogs.random_driver",
  randomTruck: "garage_manager.dialogs.random_truck",
  driverManagerTitle: "garage_manager.drivers.title",
  driverManagerSectionTitle: "garage_manager.drivers.section_title",
  driverPositions: "garage_manager.drivers.positions",
  driverPositionStatus: "garage_manager.drivers.position_status",
  freeDriverPositionsStatus: "garage_manager.drivers.free_positions_status",
  howManyDrivers: "garage_manager.drivers.select_count",
  selectedDrivers: "garage_manager.drivers.selected_count",
  maximumAvailableDrivers: "garage_manager.drivers.maximum_available",
  fillAllDriverPositions: "garage_manager.drivers.fill_all",
  currentDrivers: "garage_manager.drivers.current",
  garageFullyStaffed: "garage_manager.drivers.full",
  garageFullyStaffedMessage: "garage_manager.drivers.full_message",
  driverPoolEmpty: "garage_manager.drivers.pool_empty",
  driverPoolEmptyMessage: "garage_manager.drivers.pool_empty_message",
  driverGarageNotOwned: "garage_manager.drivers.not_owned",
  assignOneDriver: "garage_manager.drivers.assign_one",
  assignManyDrivers: "garage_manager.drivers.assign_many",
  driverAssignSuccess: "garage_manager.drivers.success",
  driverNoTruckWarning: "garage_manager.drivers.no_truck_warning",
  increaseDriverCount: "garage_manager.drivers.increase_count",
  decreaseDriverCount: "garage_manager.drivers.decrease_count",
  aiDriver: "garage_manager.drivers.ai_driver",
  driverSlot: "garage_manager.drivers.slot",
  emptyDriverSlot: "garage_manager.drivers.empty_slot",
  assignedStatus: "garage_manager.drivers.assigned",
  availableStatus: "garage_manager.drivers.available",
  assignedInGarageStatus: "garage_manager.drivers.assigned_in",
  selectAiDriver: "garage_manager.drivers.select_driver",
  searchDriversPlaceholder: "garage_manager.drivers.search_placeholder",
  availableDriversCount: "garage_manager.drivers.available_count",
  driverReference: "garage_manager.drivers.driver_reference",
  noDriverMatches: "garage_manager.drivers.no_matches",
  manualDriverAssignSuccess: "garage_manager.drivers.manual_success",
  backupCreated: "garage_manager.results.backup_created",
  backupNotCreated: "garage_manager.results.backup_not_created",
  verified: "garage_manager.results.verified",
  notVerified: "garage_manager.results.not_verified",
  noOperationResult: "garage_manager.results.no_operation",
  purchaseSuccess: "garage_manager.success.purchase",
  purchaseAllSuccess: "garage_manager.success.purchase_all",
  purchaseAllNone: "garage_manager.success.purchase_all_none",
  relinquishEmptySuccess: "garage_manager.success.relinquish_empty",
  relinquishEmptyNone: "garage_manager.success.relinquish_empty_none",
  relinquishSuccess: "garage_manager.success.relinquish",
  assignResourcesSuccess: "garage_manager.success.assign_resources",
  assignDriversSuccess: "garage_manager.success.assign_drivers",
  upgradeSuccess: "garage_manager.success.upgrade",
  downgradeSuccess: "garage_manager.success.downgrade",
  headquartersSuccess: "garage_manager.success.headquarters",
  noExtrasSuccess: "garage_manager.success.no_extras",
  updateSuccess: "garage_manager.success.update",
};

const PERSISTED_VIEW_STATE = {
  citySearch: "",
  idSearch: "",
  ownership: "all",
  size: "all",
  hq: "all",
  occupancy: "all",
  sort: "city",
  selectedGarageId: null,
};

const ACTIVE_MODALS = new WeakMap();
let modalId = 0;

const ERROR_KEYS = [
  ["profile_not_loaded", "garage_manager.errors.profile_not_loaded"],
  ["save_not_loaded", "garage_manager.errors.save_not_loaded"],
  ["game_sii_not_found", "garage_manager.errors.game_sii_not_found"],
  ["game_sii_not_decrypted", "garage_manager.errors.game_sii_not_decrypted"],
  ["garage_not_found", "garage_manager.errors.garage_not_found"],
  ["garage_already_owned", "garage_manager.errors.garage_already_owned"],
  ["garage_not_owned", "garage_manager.errors.garage_not_owned"],
  ["garage_relinquish_headquarters", "garage_manager.errors.garage_relinquish_headquarters"],
  ["garage_relinquish_not_empty", "garage_manager.errors.garage_relinquish_not_empty"],
  ["garage_relinquish_external_reference", "garage_manager.errors.garage_relinquish_external_reference"],
  ["garage_already_maximum_size", "garage_manager.errors.garage_already_maximum_size"],
  ["garage_capacity_mismatch", "garage_manager.errors.garage_capacity_mismatch"],
  ["garage_has_unresolved_references", "garage_manager.errors.garage_has_unresolved_references"],
  ["garage_state_invalid", "garage_manager.errors.garage_state_invalid"],
  ["garage_size_invalid", "garage_manager.errors.garage_size_invalid"],
  ["garage_downgrade_capacity_exceeded", "garage_manager.errors.garage_downgrade_capacity_exceeded"],
  ["garage_downgrade_has_unresolved_references", "garage_manager.errors.garage_downgrade_has_unresolved_references"],
  ["garage_size_already_selected", "garage_manager.errors.garage_size_already_selected"],
  ["garage_size_change_not_verified", "garage_manager.errors.garage_size_change_not_verified"],
  ["garage_mutation_in_progress", "garage_manager.errors.garage_mutation_in_progress"],
  ["garage_mutation_lock_unavailable", "garage_manager.errors.garage_mutation_lock_unavailable"],
  ["garage_assignment_empty", "garage_manager.errors.garage_assignment_empty"],
  ["garage_assignment_no_free_vehicle_slot", "garage_manager.errors.garage_assignment_no_free_vehicle_slot"],
  ["garage_assignment_no_available_truck", "garage_manager.errors.garage_assignment_no_available_truck"],
  ["garage_assignment_no_free_driver_slot", "garage_manager.errors.garage_assignment_no_free_driver_slot"],
  ["garage_assignment_no_available_driver", "garage_manager.errors.garage_assignment_no_available_driver"],
  ["garage_assignment_driver_count_invalid", "garage_manager.errors.garage_assignment_driver_count_invalid"],
  ["driver_pool_invalid", "garage_manager.errors.driver_pool_invalid"],
  ["garage_update_empty", "garage_manager.errors.garage_update_empty"],
  ["save_changed_since_load", "garage_manager.errors.save_changed_since_load"],
  ["backup_failed", "garage_manager.errors.backup_failed"],
  ["save_write_failed", "garage_manager.errors.save_write_failed"],
  ["save_verification_failed", "garage_manager.errors.save_verification_failed"],
  ["rollback_failed", "garage_manager.errors.rollback_failed"],
  ["garage_update_not_supported", "garage_manager.errors.game_not_supported"],
  ["garage_reference_ambiguous", "garage_manager.errors.reference_ambiguous"],
  ["garage_block_invalid", "garage_manager.errors.invalid_save"],
];

async function loadCopy() {
  const entries = await Promise.all(
    Object.entries(COPY_KEYS).map(async ([name, key]) => [name, await window.t(key)]),
  );
  return Object.fromEntries(entries);
}

function escapeHtml(value) {
  const element = document.createElement("span");
  element.textContent = String(value ?? "");
  return element.innerHTML;
}

function errorCode(error) {
  return String(error?.message || error || "");
}

function errorTranslationKey(error) {
  const code = errorCode(error);
  return ERROR_KEYS.find(([prefix]) => code.includes(prefix))?.[1]
    || "garage_manager.errors.generic";
}

function garageCity(garage, copy) {
  return garage.cityName || garage.cityToken || garage.garageId || copy.unknown;
}

function sizeLabel(size, copy) {
  const labels = {
    unowned: copy.sizeUnowned,
    small: copy.sizeSmall,
    large: copy.sizeLarge,
    unknown: copy.sizeUnknown,
  };
  return labels[size] || copy.sizeUnknown;
}

function ownershipLabel(ownership, copy) {
  if (ownership === "owned") return copy.ownershipOwnedValue;
  if (ownership === "not_owned") return copy.ownershipNotOwnedValue;
  return copy.ownershipUnknown;
}

function formatCopy(template, values = {}) {
  return Object.entries(values).reduce(
    (text, [name, value]) => text.replaceAll(`{${name}}`, String(value)),
    String(template || ""),
  );
}

function slotCountLabel(count, copy) {
  return formatCopy(copy.slotCount, { count: Number(count || 0) });
}

function garageIsBlocked(garage) {
  return !garage.capacityConsistent
    || (garage.warnings || []).some((warning) => (
      warning.includes("_reference_unresolved")
      || warning.includes("_reference_ambiguous")
      || warning.includes("_reference_duplicate")
      || warning.includes("_slot_assignment_inconsistent")
    ));
}

function canDowngradeGarage(garage) {
  const removedSlotOccupied = (garage.slots || []).some((slot) => (
    Number(slot.index) >= 3 && (slot.truckId || slot.driverId)
  ));
  return garage.size === "large"
    && !garageIsBlocked(garage)
    && Number(garage.occupiedSlots || 0) <= 3
    && !removedSlotOccupied;
}

function canRelinquishGarage(garage) {
  return garage.ownership === "owned"
    && !garage.isHeadquarters
    && !garageIsBlocked(garage)
    && Number(garage.occupiedSlots || 0) === 0
    && Number(garage.assignedTruckCount || 0) === 0
    && Number(garage.assignedDriverCount || 0) === 0
    && Number(garage.assignedTrailerCount || 0) === 0
    && Number(garage.trailerSlotCount || 0) === 0;
}

function freeDriverSlotCount(garage) {
  return Math.max(0, Number(garage?.driverSlotCount || 0) - Number(garage?.assignedDriverCount || 0));
}
function driverAssignmentLimit(garage, driverPool) {
  return Math.min(
    freeDriverSlotCount(garage),
    Math.max(0, Number(driverPool?.availableDriverCount || 0)),
  );
}

function clampDriverSelection(value, maximum) {
  if (maximum <= 0) return 0;
  return Math.min(maximum, Math.max(1, Number(value || 1)));
}

function driverAssignButtonLabel(count, copy) {
  return count === 1
    ? copy.assignOneDriver
    : formatCopy(copy.assignManyDrivers, { count });
}

function driversWithoutTruckCount(garage) {
  return Math.max(
    0,
    Number(garage?.assignedDriverCount || 0) - Number(garage?.assignedTruckCount || 0),
  );
}

function formatDriverPoolAvailability(driverPool, copy) {
  if (!driverPool) return copy.notAvailable;
  return formatCopy(copy.occupancyValue, {
    occupied: Number(driverPool.availableDriverCount || 0),
    capacity: Number(driverPool.driverPoolCount || 0),
  });
}

function garageForDriver(driverId, garages) {
  const normalized = String(driverId || "");
  return (garages || []).find((garage) => (garage.slots || []).some(
    (slot) => slot.driverId === normalized,
  ));
}

function driverPoolEntries(driverPool, garages, copy) {
  const available = new Set(driverPool?.availableDriverIds || []);
  const assigned = new Set(driverPool?.assignedDriverIds || []);
  const entries = new Map();
  (driverPool?.drivers || []).forEach((driver) => {
    if (driver?.driverId) {
      entries.set(driver.driverId, {
        driverId: driver.driverId,
        index: driver.index,
      });
    }
  });
  assigned.forEach((driverId) => {
    if (!entries.has(driverId)) {
      entries.set(driverId, { driverId, index: null });
    }
  });
  return Array.from(entries.values()).map((entry) => {
    const assignedGarage = garageForDriver(entry.driverId, garages);
    const isAvailable = available.has(entry.driverId);
    const isAssigned = assigned.has(entry.driverId);
    const status = isAvailable
      ? copy.availableStatus
      : assignedGarage
        ? formatCopy(copy.assignedInGarageStatus, { city: garageCity(assignedGarage, copy) })
        : isAssigned
          ? copy.assignedStatus
          : copy.notAvailable;
    return {
      ...entry,
      isAvailable,
      isAssigned,
      status,
      searchText: [entry.driverId, status].join(" ").toLowerCase(),
    };
  }).sort((left, right) => {
    if (left.isAvailable !== right.isAvailable) return left.isAvailable ? -1 : 1;
    if (left.isAssigned !== right.isAssigned) return left.isAssigned ? 1 : -1;
    return left.driverId.localeCompare(right.driverId);
  });
}

function garageDriverSlotListMarkup(garage, copy, assignDisabled) {
  const slotCount = Math.max(
    Number(garage?.driverSlotCount || 0),
    (garage?.slots || []).length,
  );
  const slots = new Map((garage?.slots || []).map((slot) => [Number(slot.index), slot]));
  if (slotCount <= 0) {
    return "<p class='garage-reference-empty'>" + escapeHtml(copy.noDrivers) + "</p>";
  }
  return "<div class='garage-driver-slot-list'>"
    + Array.from({ length: slotCount }, (_, index) => {
      const slot = slots.get(index) || { index, driverId: null, truckId: null };
      const driverId = slot.driverId;
      const title = driverId ? copy.aiDriver : copy.emptyDriverSlot;
      const ref = driverId || formatCopy(copy.driverSlot, { index });
      const status = driverId ? copy.assignedStatus : copy.availableStatus;
      const action = driverId
        ? ""
        : "<button type='button' class='button-secondary' data-garage-driver-slot-assign data-garage-driver-slot-index='"
          + escapeHtml(index)
          + "'"
          + (assignDisabled ? " disabled" : "")
          + ">"
          + escapeHtml(copy.assignDriver)
          + "</button>";
      return "<article class='garage-driver-slot-card"
        + (driverId ? " is-assigned" : " is-empty")
        + "'><div><strong>"
        + escapeHtml(title)
        + "</strong><code>"
        + escapeHtml(ref)
        + "</code><span>"
        + escapeHtml(status)
        + "</span></div>"
        + action
        + "</article>";
    }).join("")
    + "</div>";
}
function technicalMessage(error, copy) {
  let message = errorCode(error);
  [window.selectedProfilePath, window.selectedSavePath, window.currentSavePath]
    .filter(Boolean)
    .forEach((path) => {
      message = message.replaceAll(String(path), copy.pathHidden);
    });
  return message;
}

function summaryCard(label, value) {
  return "<article class='garage-summary-card'><span>"
    + escapeHtml(label)
    + "</span><strong>"
    + escapeHtml(value)
    + "</strong></article>";
}

function metric(label, value) {
  return "<div class='garage-metric'><span>"
    + escapeHtml(label)
    + "</span><strong>"
    + escapeHtml(value)
    + "</strong></div>";
}

function detailRow(label, value) {
  return "<div><dt>"
    + escapeHtml(label)
    + "</dt><dd>"
    + escapeHtml(value)
    + "</dd></div>";
}

function actionButton(action, garageId, label, className = "", disabled = false) {
  return "<button type='button' class='"
    + escapeHtml(className)
    + "' data-garage-action='"
    + escapeHtml(action)
    + "' data-garage-id='"
    + escapeHtml(garageId)
    + "'"
    + (disabled ? " disabled" : "")
    + ">"
    + escapeHtml(label)
    + "</button>";
}

function createModal(root, {
  title,
  subtitle = "",
  bodyMarkup,
  footerMarkup = "",
  copy,
  className = "",
  returnFocus = null,
  canClose = null,
}) {
  ACTIVE_MODALS.get(root)?.close({ restoreFocus: false, force: true });
  const previouslyFocused = returnFocus || (
    document.activeElement instanceof HTMLElement ? document.activeElement : null
  );
  const overlay = document.createElement("div");
  overlay.className = "garage-modal";
  const titleId = `garage-modal-title-${++modalId}`;
  const subtitleMarkup = subtitle
    ? "<p class='garage-modal-subtitle'>" + escapeHtml(subtitle) + "</p>"
    : "";
  const footer = footerMarkup
    ? "<footer class='garage-modal-footer'>" + footerMarkup + "</footer>"
    : "";
  overlay.innerHTML = "<section class='garage-modal-box "
    + escapeHtml(className)
    + "' role='dialog' aria-modal='true' aria-labelledby='"
    + titleId
    + "'><header><div><span class='overview-label'>"
    + escapeHtml(copy.title)
    + "</span><h2 id='"
    + titleId
    + "'>"
    + escapeHtml(title)
    + "</h2>"
    + subtitleMarkup
    + "</div><button type='button' class='garage-modal-close' data-garage-modal-close aria-label='"
    + escapeHtml(copy.close)
    + "'><span aria-hidden='true'>×</span><span>"
    + escapeHtml(copy.close)
    + "</span></button></header><div class='garage-modal-body'>"
    + bodyMarkup
    + "</div>"
    + footer
    + "</section>";
  document.body.appendChild(overlay);

  let closed = false;
  const host = root.parentElement;
  const observer = new MutationObserver(() => {
    if (!root.isConnected) close({ restoreFocus: false, force: true });
  });
  if (host) observer.observe(host, { childList: true });
  const close = ({ restoreFocus = true, force = false } = {}) => {
    if (closed) return;
    if (!force && typeof canClose === "function" && !canClose()) return;
    closed = true;
    document.removeEventListener("keydown", handleKeydown);
    observer.disconnect();
    overlay.remove();
    if (ACTIVE_MODALS.get(root)?.overlay === overlay) {
      ACTIVE_MODALS.delete(root);
    }
    if (restoreFocus) {
      if (typeof previouslyFocused === "function") {
        previouslyFocused();
      } else if (previouslyFocused?.isConnected) {
        previouslyFocused.focus();
      }
    }
  };
  const focusableElements = () => Array.from(overlay.querySelectorAll(
    "button:not([disabled]), input:not([disabled]), select:not([disabled]), "
      + "textarea:not([disabled]), a[href], summary, [tabindex]:not([tabindex='-1'])",
  )).filter((element) => element.getClientRects().length > 0);
  const handleKeydown = (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }
    if (event.key !== "Tab") return;
    const focusable = focusableElements();
    if (!focusable.length) {
      event.preventDefault();
      overlay.querySelector(".garage-modal-box")?.focus();
      return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };
  overlay.querySelector("[data-garage-modal-close]")?.addEventListener("click", close);
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) close();
  });
  document.addEventListener("keydown", handleKeydown);
  overlay.querySelector("[data-garage-modal-close]")?.focus();
  const modal = { overlay, close, returnFocus: previouslyFocused };
  ACTIVE_MODALS.set(root, modal);
  return modal;
}

function assignmentSection(label, values, emptyText) {
  const items = values.length
    ? "<ul class='garage-reference-list'>"
      + values.map((value) => "<li><code>" + escapeHtml(value) + "</code></li>").join("")
      + "</ul>"
    : "<p class='garage-reference-empty'>" + escapeHtml(emptyText) + "</p>";
  return "<details class='garage-assignment-section'><summary>"
    + escapeHtml(label)
    + "</summary>"
    + items
    + "</details>";
}

export async function mountGarageManager(container) {
  const root = document.createElement("section");
  root.className = "garage-manager";
  container.appendChild(root);
  const copy = await loadCopy();
  if (!root.isConnected) return;

  const state = {
    result: null,
    driverPool: null,
    driverPoolError: null,
    loading: false,
    mutationPending: false,
    bulkMutationPending: false,
    bulkMutationOperation: null,
    error: null,
    refreshError: null,
    lastMutationResult: null,
    lastDriverAssignmentResult: null,
    highlightedGarageId: null,
    ...PERSISTED_VIEW_STATE,
  };

  const profileName = window.selectedProfilePath
    ? document.querySelector("#profileNameDisplay")?.textContent?.trim() || copy.noProfileShort
    : copy.noProfileShort;
  const saveName = window.selectedSavePath
    ? document.querySelector("#saveName")?.textContent?.trim() || copy.noSaveShort
    : copy.noSaveShort;
  root.innerHTML = "<header class='garage-manager-head'><div><span class='overview-label'>"
    + escapeHtml(copy.title)
    + "</span><h2>"
    + escapeHtml(copy.title)
    + "</h2><p>"
    + escapeHtml(copy.description)
    + "</p><dl class='garage-context'>"
    + detailRow(copy.profileLabel, profileName)
    + detailRow(copy.saveLabel, saveName)
    + "</dl></div><div class='garage-manager-actions'><button type='button' data-garage-sell-empty>"
    + escapeHtml(copy.relinquishEmpty)
    + "</button><button type='button' data-garage-buy-all>"
    + escapeHtml(copy.purchaseAll)
    + "</button><button type='button' class='garage-refresh-button' data-garage-refresh>"
    + escapeHtml(copy.refresh)
    + "</button></div></header><div class='garage-summary-grid' data-garage-summary></div>"
    + "<section class='garage-filter-panel'><div class='garage-toolbar'>"
    + "<label class='garage-filter garage-filter--city'><span>"
    + escapeHtml(copy.citySearchLabel)
    + "</span><input type='search' data-garage-search='citySearch' value='"
    + escapeHtml(state.citySearch)
    + "' placeholder='"
    + escapeHtml(copy.citySearchPlaceholder)
    + "'></label><label class='garage-filter garage-filter--id'><span>"
    + escapeHtml(copy.idSearchLabel)
    + "</span><input type='search' data-garage-search='idSearch' value='"
    + escapeHtml(state.idSearch)
    + "' placeholder='"
    + escapeHtml(copy.idSearchPlaceholder)
    + "'></label>"
    + selectMarkup("ownership", copy.ownershipLabel, [
      ["all", copy.ownershipAll],
      ["owned", copy.ownershipOwned],
      ["not_owned", copy.ownershipNotOwned],
    ], state.ownership)
    + selectMarkup("size", copy.sizeLabelFilter, [
      ["all", copy.sizeAll],
      ["unowned", copy.sizeUnownedFilter],
      ["small", copy.sizeSmallFilter],
      ["large", copy.sizeLargeFilter],
    ], state.size)
    + selectMarkup("hq", copy.hqLabel, [
      ["all", copy.hqAll],
      ["hq", copy.hqOnly],
      ["not_hq", copy.hqExclude],
    ], state.hq)
    + selectMarkup("occupancy", copy.occupancyLabel, [
      ["all", copy.occupancyAll],
      ["free", copy.occupancyFree],
      ["full", copy.occupancyFull],
    ], state.occupancy)
    + selectMarkup("sort", copy.sortLabel, [
      ["city", copy.sortCity],
      ["size", copy.sortSize],
      ["free", copy.sortFree],
      ["drivers", copy.sortDrivers],
      ["trucks", copy.sortTrucks],
    ], state.sort)
    + "</div><footer class='garage-filter-footer'><strong data-garage-filter-status></strong>"
    + "<button type='button' class='button-secondary' data-garage-filter-reset>"
    + escapeHtml(copy.resetFilters)
    + "</button></footer></section>"
    + "<div class='garage-notice' data-garage-notice hidden></div>"
    + "<div class='garage-result-notice' data-garage-result hidden></div>"
    + "<div class='garage-list' data-garage-list></div>";

  const summaryElement = root.querySelector("[data-garage-summary]");
  const listElement = root.querySelector("[data-garage-list]");
  const noticeElement = root.querySelector("[data-garage-notice]");
  const resultElement = root.querySelector("[data-garage-result]");
  const refreshButton = root.querySelector("[data-garage-refresh]");
  const sellEmptyButton = root.querySelector("[data-garage-sell-empty]");
  const buyAllButton = root.querySelector("[data-garage-buy-all]");
  const filterStatusElement = root.querySelector("[data-garage-filter-status]");
  const resetFiltersButton = root.querySelector("[data-garage-filter-reset]");

  function selectMarkup(name, label, options, selectedValue) {
    return "<label class='garage-filter'><span>"
      + escapeHtml(label)
      + "</span><select data-garage-filter='"
      + escapeHtml(name)
      + "'>"
      + options.map(([value, text]) => "<option value='"
        + escapeHtml(value)
        + "'"
        + (value === selectedValue ? " selected" : "")
        + ">"
        + escapeHtml(text)
        + "</option>").join("")
      + "</select></label>";
  }

  function allGarages() {
    return Array.isArray(state.result?.garages) ? state.result.garages : [];
  }

  function findGarage(garageId) {
    return allGarages().find((garage) => garage.garageId === garageId);
  }

  function persistViewState() {
    Object.assign(PERSISTED_VIEW_STATE, {
      citySearch: state.citySearch,
      idSearch: state.idSearch,
      ownership: state.ownership,
      size: state.size,
      hq: state.hq,
      occupancy: state.occupancy,
      sort: state.sort,
      selectedGarageId: state.selectedGarageId,
    });
  }

  function activeFilterCount() {
    return [
      state.citySearch.trim(),
      state.idSearch.trim(),
      state.ownership !== "all",
      state.size !== "all",
      state.hq !== "all",
      state.occupancy !== "all",
    ].filter(Boolean).length;
  }

  function renderFilterStatus() {
    const count = activeFilterCount();
    filterStatusElement.textContent = formatCopy(
      count === 1 ? copy.activeFilterOne : copy.activeFilterMany,
      { count },
    );
    filterStatusElement.classList.toggle("is-active", count > 0);
    resetFiltersButton.disabled = count === 0;
  }

  function renderSummary() {
    if (state.loading && !state.result) {
      summaryElement.innerHTML = Array.from(
        { length: 10 },
        () => "<article class='garage-summary-card is-skeleton' aria-hidden='true'></article>",
      ).join("");
      return;
    }
    const garages = allGarages();
    const owned = garages.filter((garage) => garage.ownership === "owned");
    const values = [
      [copy.total, garages.length],
      [copy.owned, owned.length],
      [copy.notOwned, garages.filter((garage) => garage.ownership === "not_owned").length],
      [copy.smallGarages, owned.filter((garage) => garage.size === "small").length],
      [copy.largeGarages, owned.filter((garage) => garage.size === "large").length],
      [copy.freeSlots, owned.reduce((sum, garage) => sum + Number(garage.availableSlots || 0), 0)],
      [
        copy.occupiedSlotsSummary,
        owned.reduce((sum, garage) => sum + Number(garage.occupiedSlots || 0), 0),
      ],
      [copy.drivers, owned.reduce((sum, garage) => sum + Number(garage.assignedDriverCount || 0), 0)],
      [copy.aiDriverPool, formatDriverPoolAvailability(state.driverPool, copy)],
      [copy.trucks, owned.reduce((sum, garage) => sum + Number(garage.assignedTruckCount || 0), 0)],
      [copy.trailers, owned.reduce((sum, garage) => sum + Number(garage.assignedTrailerCount || 0), 0)],
    ];
    summaryElement.innerHTML = values.map(([label, value]) => summaryCard(label, value)).join("");
  }

  function filteredGarages() {
    const cityQuery = state.citySearch.trim().toLocaleLowerCase();
    const idQuery = state.idSearch.trim().toLocaleLowerCase();
    const sizeOrder = { unowned: 0, unknown: 1, small: 2, large: 3 };
    const garages = allGarages().filter((garage) => {
      const citySearchable = [
        garage.cityName,
        garage.cityToken,
      ].filter(Boolean).join(" ").toLocaleLowerCase();
      const idSearchable = String(garage.garageId || "").toLocaleLowerCase();
      if (cityQuery && !citySearchable.includes(cityQuery)) return false;
      if (idQuery && !idSearchable.includes(idQuery)) return false;
      if (state.ownership !== "all" && garage.ownership !== state.ownership) return false;
      if (state.size !== "all" && garage.size !== state.size) return false;
      if (state.hq === "hq" && !garage.isHeadquarters) return false;
      if (state.hq === "not_hq" && garage.isHeadquarters) return false;
      if (state.occupancy === "free"
        && (garage.ownership !== "owned" || Number(garage.availableSlots) <= 0)) return false;
      if (state.occupancy === "full"
        && (garage.ownership !== "owned" || Number(garage.availableSlots) !== 0)) return false;
      return true;
    });
    garages.sort((left, right) => {
      if (state.sort === "size") {
        return (sizeOrder[right.size] || 0) - (sizeOrder[left.size] || 0);
      }
      if (state.sort === "free") {
        return Number(right.availableSlots || 0) - Number(left.availableSlots || 0);
      }
      if (state.sort === "drivers") {
        return Number(right.assignedDriverCount || 0) - Number(left.assignedDriverCount || 0);
      }
      if (state.sort === "trucks") {
        return Number(right.assignedTruckCount || 0) - Number(left.assignedTruckCount || 0);
      }
      return garageCity(left, copy).localeCompare(garageCity(right, copy));
    });
    return garages;
  }

  function renderNotice() {
    const isReadOnly = state.result && state.result.game !== "ets2";
    const notices = [];
    if (isReadOnly) {
      notices.push("<p>" + escapeHtml(copy.atsReadOnly) + "</p>");
    }
    if (state.refreshError) {
      notices.push("<div class='garage-inline-notice'><p>"
        + escapeHtml(copy.refreshError)
        + "</p><button type='button' class='button-secondary' data-garage-retry>"
        + escapeHtml(copy.retry)
        + "</button></div>");
    }
    noticeElement.hidden = notices.length === 0;
    noticeElement.classList.toggle("is-error", Boolean(state.refreshError));
    noticeElement.innerHTML = notices.join("");
  }

  function renderResultNotice() {
    const result = state.lastMutationResult;
    resultElement.hidden = !result;
    if (!result) {
      resultElement.innerHTML = "";
      return;
    }
    const isBulkPurchase = result.operation === "purchase_all"
      && Number.isInteger(result.purchasedCount);
    const isBulkRelinquish = result.operation === "relinquish_empty"
      && Number.isInteger(result.relinquishedCount);
    const city = isBulkPurchase || isBulkRelinquish ? "" : garageCity(result.updatedState, copy);
    const successText = isBulkPurchase
      ? result.purchasedCount > 0
        ? formatCopy(copy.purchaseAllSuccess, { count: result.purchasedCount })
        : copy.purchaseAllNone
      : isBulkRelinquish
        ? result.relinquishedCount > 0
          ? formatCopy(copy.relinquishEmptySuccess, { count: result.relinquishedCount })
          : copy.relinquishEmptyNone
        : formatCopy(mutationSuccessText(result), { city });
    const detailText = isBulkRelinquish
      ? copy.relinquishEmptyEffect
      : result.operation === "assign_resources"
        ? copy.assignResourcesEffect
        : copy.noExtrasSuccess;
    resultElement.innerHTML = "<strong>"
      + escapeHtml(successText)
      + "</strong><span>"
      + escapeHtml(detailText)
      + "</span><span>"
      + escapeHtml(result.backupCreated ? copy.backupCreated : copy.backupNotCreated)
      + "</span><span>"
      + escapeHtml(result.verified ? copy.verified : copy.notVerified)
      + "</span>";
  }

  function stateCard(message, tone = "", actions = "") {
    return "<article class='garage-state-card "
      + escapeHtml(tone)
      + "'><p>"
      + escapeHtml(message)
      + "</p>"
      + actions
      + "</article>";
  }

  function renderList() {
    if (state.loading && !state.result) {
      const skeletons = Array.from(
        { length: 3 },
        () => "<article class='garage-card is-skeleton' aria-hidden='true'></article>",
      ).join("");
      listElement.innerHTML = stateCard(
        copy.loading,
        "is-loading",
        "<span class='garage-spinner' aria-hidden='true'></span><small>"
          + escapeHtml(copy.loadingHint)
          + "</small>",
      ) + skeletons;
      return;
    }
    if (!window.selectedProfilePath) {
      listElement.innerHTML = stateCard(copy.noProfile);
      return;
    }
    if (!window.selectedSavePath) {
      listElement.innerHTML = stateCard(copy.noSave);
      return;
    }
    if (state.error) {
      listElement.innerHTML = stateCard(
        copy.loadError,
        "is-error",
        "<button type='button' data-garage-retry>"
          + escapeHtml(copy.retry)
          + "</button><details><summary>"
          + escapeHtml(copy.technicalDetails)
          + "</summary><dl class='garage-error-details'>"
          + detailRow(copy.command, "get_all_garages")
          + detailRow(copy.backendMessage, technicalMessage(state.error, copy))
          + "</dl></details>",
      );
      return;
    }
    const garages = filteredGarages();
    if (!allGarages().length) {
      listElement.innerHTML = stateCard(copy.noGarages);
      return;
    }
    if (!garages.length) {
      listElement.innerHTML = stateCard(copy.noResults);
      return;
    }
    listElement.innerHTML = garages.map(renderGarageCard).join("");
  }

  function renderGarageCard(garage) {
    const owned = garage.ownership === "owned";
    const readOnly = state.result?.game !== "ets2";
    const blocked = garageIsBlocked(garage);
    const canMutate = !readOnly && !blocked && !state.mutationPending;
    const badges = (garage.isHeadquarters
      ? "<span class='garage-badge is-hq'><span aria-hidden='true'>★</span>"
        + escapeHtml(copy.hq)
        + "</span>"
      : "")
      + "<span class='garage-badge is-ownership'>"
      + escapeHtml(ownershipLabel(garage.ownership, copy))
      + "</span><span class='garage-badge is-size'>"
      + escapeHtml(sizeLabel(garage.size, copy))
      + "</span>";
    const warnings = (garage.warnings || []).length
      ? "<div class='garage-card-warning'><span aria-hidden='true'>⚠</span>"
        + escapeHtml(copy.warningStatus)
        + ": "
        + escapeHtml(garage.warnings.length)
        + "</div>"
      : "";
    let actions = "";
    if (garage.ownership === "not_owned") {
      actions += actionButton("purchase", garage.garageId, copy.purchase, "", !canMutate);
    } else if (owned && garage.size === "small") {
      actions += actionButton("upgrade", garage.garageId, copy.upgrade, "", !canMutate);
    } else if (owned) {
      actions += actionButton("details-actions", garage.garageId, copy.edit, "", false);
    }
    actions += actionButton("details", garage.garageId, copy.details, "button-secondary");
    const blockedMessage = blocked
      ? "<p class='garage-mutation-blocked'>" + escapeHtml(copy.mutationBlocked) + "</p>"
      : "";
    const classes = [
      "garage-card",
      garage.ownership === "not_owned" ? "is-unowned" : "",
      garage.isHeadquarters ? "is-headquarters" : "",
      blocked ? "has-warning" : "",
      state.selectedGarageId === garage.garageId ? "is-selected" : "",
      state.highlightedGarageId === garage.garageId ? "is-highlighted" : "",
    ].filter(Boolean).join(" ");
    const occupancyMetric = owned
      ? formatCopy(copy.occupancyValue, {
        occupied: garage.occupiedSlots,
        capacity: garage.vehicleSlotCount,
      })
      : slotCountLabel(0, copy);
    return "<article class='"
      + classes
      + "' data-garage-card-id='"
      + escapeHtml(garage.garageId)
      + "'><header><div><span class='garage-city-token'>"
      + escapeHtml(garage.countryCode || copy.notAvailable)
      + "</span><h3>"
      + escapeHtml(garageCity(garage, copy))
      + "</h3></div><div class='garage-badges'>"
      + badges
      + "</div></header><div class='garage-metrics'>"
      + metric(copy.occupiedSlots, occupancyMetric)
      + metric(
        copy.assignedDrivers,
        owned
          ? formatCopy(copy.occupancyValue, {
            occupied: garage.assignedDriverCount,
            capacity: garage.driverSlotCount,
          })
          : garage.assignedDriverCount,
      )
      + metric(
        copy.assignedTrucks,
        owned
          ? formatCopy(copy.occupancyValue, {
            occupied: garage.assignedTruckCount,
            capacity: garage.vehicleSlotCount,
          })
          : garage.assignedTruckCount,
      )
      + metric(copy.assignedTrailers, garage.assignedTrailerCount)
      + "</div>"
      + warnings
      + blockedMessage
      + "<footer class='garage-card-actions'>"
      + actions
      + "</footer></article>";
  }

  function render() {
    renderSummary();
    renderFilterStatus();
    renderNotice();
    renderResultNotice();
    renderList();
    root.setAttribute("aria-busy", state.loading || state.mutationPending ? "true" : "false");
    refreshButton.disabled = state.loading || state.mutationPending;
    refreshButton.textContent = state.loading ? copy.refreshing : copy.refresh;
    sellEmptyButton.disabled = state.loading
      || state.mutationPending
      || !state.result
      || state.result.game !== "ets2";
    sellEmptyButton.textContent = state.bulkMutationOperation === "relinquish_empty"
      ? copy.relinquishingEmpty
      : copy.relinquishEmpty;
    buyAllButton.disabled = state.loading
      || state.mutationPending
      || !state.result
      || state.result.game !== "ets2";
    buyAllButton.textContent = state.bulkMutationOperation === "purchase_all"
      ? copy.purchasingAll
      : copy.purchaseAll;
  }

  async function loadGarages({
    toastOnError = false,
    expectedSaveHash = null,
    preserveOnError = false,
  } = {}) {
    const previousResult = state.result;
    const scrollContainer = root.closest(".save-content-scroll");
    const previousScrollTop = scrollContainer?.scrollTop;
    state.error = null;
    state.refreshError = null;
    state.driverPoolError = null;
    if (!window.selectedProfilePath || !window.selectedSavePath) {
      state.result = null;
      state.driverPool = null;
      render();
      return null;
    }
    state.loading = true;
    render();
    try {
      const result = await window.invoke("get_all_garages");
      let driverPool = null;
      if (result?.game === "ets2") {
        try {
          driverPool = await window.invoke("get_ai_driver_pool");
        } catch (poolError) {
          console.warn("AI driver pool load failed:", poolError);
          state.driverPoolError = poolError;
        }
      }
      if (expectedSaveHash && result?.saveHash !== expectedSaveHash) {
        throw new Error("garage_size_change_not_verified:save_hash");
      }
      state.result = result;
      state.driverPool = driverPool;
      if (state.selectedGarageId && !findGarage(state.selectedGarageId)) {
        state.selectedGarageId = null;
        persistViewState();
      }
      return result;
    } catch (error) {
      console.error("Garage list load failed:", error);
      state.error = preserveOnError ? null : error;
      state.refreshError = preserveOnError ? error : null;
      state.result = preserveOnError ? previousResult : null;
      state.driverPool = preserveOnError ? state.driverPool : null;
      if (toastOnError) {
        window.showToast(errorTranslationKey(error), {}, "error");
      }
      return null;
    } finally {
      state.loading = false;
      if (root.isConnected) {
        render();
        if (scrollContainer && Number.isFinite(previousScrollTop)) {
          requestAnimationFrame(() => {
            scrollContainer.scrollTop = previousScrollTop;
          });
        }
      }
    }
  }

  function validateMutationResult(result) {
    if (!result?.verified || !result?.saveHash || !result?.updatedState) {
      throw new Error("garage_size_change_not_verified:response");
    }
    if (result.garageId !== result.updatedState.garageId) {
      throw new Error("garage_size_change_not_verified:garage_id");
    }
    const garages = allGarages();
    if (!garages.some((garage) => garage.garageId === result.garageId)) {
      throw new Error("garage_size_change_not_verified:garage_missing");
    }
  }

  function validateBulkMutationResult(result, operation) {
    const idField = operation === "purchase_all" ? "purchasedGarageIds" : "relinquishedGarageIds";
    const countField = operation === "purchase_all" ? "purchasedCount" : "relinquishedCount";
    if (result?.operation !== operation
      || !result.verified
      || !result.saveHash
      || !Number.isInteger(result[countField])
      || !Array.isArray(result[idField])
      || result[idField].length !== result[countField]) {
      throw new Error("garage_size_change_not_verified:batch_response");
    }
    const knownGarageIds = new Set(allGarages().map((garage) => garage.garageId));
    if (result[idField].some((garageId) => !knownGarageIds.has(garageId))) {
      throw new Error("garage_size_change_not_verified:garage_missing");
    }
  }

  function mutationSuccessKey(result) {
    const previous = result.previousState;
    const updated = result.updatedState;
    if (result.operation === "assign_resources") {
      return "garage_manager.success.assign_resources";
    }
    if (result.operation === "relinquish"
      && previous?.ownership === "owned"
      && updated?.ownership === "not_owned"
      && updated?.status === 0) {
      return "garage_manager.success.relinquish";
    }
    if (previous?.ownership === "not_owned"
      && updated?.ownership === "owned"
      && updated?.status === 3
      && updated?.vehicleSlotCount === 5
      && updated?.driverSlotCount === 5) {
      return "garage_manager.success.purchase";
    }
    if (previous?.size === "small"
      && updated?.size === "large"
      && updated?.status === 3
      && updated?.vehicleSlotCount === 5
      && updated?.driverSlotCount === 5) {
      return "garage_manager.success.upgrade";
    }
    if (previous?.size === "large"
      && updated?.size === "small"
      && updated?.status === 2
      && updated?.vehicleSlotCount === 3
      && updated?.driverSlotCount === 3) {
      return "garage_manager.success.downgrade";
    }
    if (!previous?.isHeadquarters && updated?.isHeadquarters) {
      return "garage_manager.success.headquarters";
    }
    return "garage_manager.success.update";
  }

  function mutationSuccessText(result) {
    const key = mutationSuccessKey(result);
    if (key === "garage_manager.success.purchase") return copy.purchaseSuccess;
    if (key === "garage_manager.success.relinquish") return copy.relinquishSuccess;
    if (key === "garage_manager.success.assign_resources") return copy.assignResourcesSuccess;
    if (key === "garage_manager.success.upgrade") return copy.upgradeSuccess;
    if (key === "garage_manager.success.downgrade") return copy.downgradeSuccess;
    if (key === "garage_manager.success.headquarters") return copy.headquartersSuccess;
    return copy.updateSuccess;
  }

  function focusGarageAction(garageId, preferredAction) {
    requestAnimationFrame(() => {
      const buttons = Array.from(root.querySelectorAll("[data-garage-action]"));
      const target = buttons.find((button) => (
        button.dataset.garageId === garageId
        && button.dataset.garageAction === preferredAction
      )) || buttons.find((button) => button.dataset.garageId === garageId);
      target?.focus();
      target?.closest("[data-garage-card-id]")?.scrollIntoView({
        block: "nearest",
        inline: "nearest",
      });
    });
  }

  function openDetails(garageId, {
    focusActions = false,
    returnFocus = null,
  } = {}) {
    const garage = findGarage(garageId);
    if (!garage) return;
    state.selectedGarageId = garage.garageId;
    persistViewState();
    renderList();
    const driverIds = (garage.slots || []).map((slot) => slot.driverId).filter(Boolean);
    const truckIds = (garage.slots || []).map((slot) => slot.truckId).filter(Boolean);
    const readOnly = state.result?.game !== "ets2";
    const blocked = garageIsBlocked(garage);
    const lastResult = state.lastMutationResult?.garageId === garage.garageId
      ? state.lastMutationResult
      : null;
    const writeStatus = readOnly
      ? copy.writeReadOnly
      : blocked
        ? copy.writeBlocked
        : copy.writeReady;
    const warningItems = (garage.warnings || []).length
      ? "<ul class='garage-reference-list'>"
        + garage.warnings.map((warning) => "<li><code>" + escapeHtml(warning) + "</code></li>").join("")
        + "</ul>"
      : "<p class='garage-reference-empty'>" + escapeHtml(copy.noWarnings) + "</p>";
    const downgradeBlockedMessage = garage.size === "large" && !canDowngradeGarage(garage)
      ? "<p class='garage-mutation-blocked'>"
        + escapeHtml(formatCopy(copy.downgradeBlocked, {
          occupied: garage.occupiedSlots,
          capacity: garage.vehicleSlotCount,
        }))
        + "</p>"
      : blocked
        ? "<p class='garage-mutation-blocked'>" + escapeHtml(copy.mutationBlocked) + "</p>"
        : "";
    const relinquishBlockedMessage = garage.ownership === "owned"
      && !garage.isHeadquarters
      && !canRelinquishGarage(garage)
      ? "<p class='garage-mutation-blocked'>" + escapeHtml(copy.relinquishBlocked) + "</p>"
      : "";
    const actionBlockedMessage = downgradeBlockedMessage + relinquishBlockedMessage;
    const freeDriverSlots = freeDriverSlotCount(garage);
    const availableAiDrivers = Math.max(0, Number(state.driverPool?.availableDriverCount || 0));
    const driverManagerDisabled = readOnly
      || blocked
      || state.mutationPending
      || garage.ownership !== "owned";
    const driverPreviewState = garage.ownership !== "owned"
      ? copy.driverGarageNotOwned
      : freeDriverSlots <= 0
        ? copy.garageFullyStaffed
        : availableAiDrivers <= 0
          ? copy.driverPoolEmpty
          : formatCopy(copy.freeDriverPositionsStatus, { count: freeDriverSlots });
    const driverNoTruckCount = driversWithoutTruckCount(garage);
    const driverNoTruckNotice = driverNoTruckCount > 0
      ? "<p class='garage-driver-note'>"
        + escapeHtml(formatCopy(copy.driverNoTruckWarning, { count: driverNoTruckCount }))
        + "</p>"
      : "";
    const randomDriverDisabled = driverManagerDisabled
      || freeDriverSlots <= 0
      || availableAiDrivers <= 0;
    const driverSlotList = garageDriverSlotListMarkup(garage, copy, randomDriverDisabled);
    const driverPreviewSection = "<section class='garage-detail-section garage-driver-preview'><div class='garage-driver-preview-head'><div><h3>"
      + escapeHtml(copy.driverManagerSectionTitle)
      + "</h3><p>"
      + escapeHtml(formatCopy(copy.driverPositionStatus, {
        assigned: garage.assignedDriverCount,
        capacity: garage.driverSlotCount,
      }))
      + "</p></div><div class='garage-driver-preview-actions'><button type='button' class='button-secondary' data-garage-detail-assign-random-drivers"
      + (randomDriverDisabled ? " disabled" : "")
      + ">"
      + escapeHtml(copy.assignRandomDrivers)
      + "</button><button type='button' class='button-secondary' data-garage-detail-driver-manager"
      + (driverManagerDisabled ? " disabled" : "")
      + ">"
      + escapeHtml(copy.manageDrivers)
      + "</button></div></div><dl class='garage-detail-list'>"
      + detailRow(copy.assignedDrivers, formatCopy(copy.occupancyValue, {
        occupied: garage.assignedDriverCount,
        capacity: garage.driverSlotCount,
      }))
      + detailRow(copy.freeDriverPositions, freeDriverSlots)
      + detailRow(copy.availableAiDrivers, availableAiDrivers)
      + "</dl>"
      + driverSlotList
      + "<p class='garage-reference-empty'>"
      + escapeHtml(driverPreviewState)
      + "</p>"
      + driverNoTruckNotice
      + "</section>";
    const body = "<section class='garage-detail-section'><h3>"
      + escapeHtml(copy.generalInformation)
      + "</h3><dl class='garage-detail-list'>"
      + detailRow(copy.status, ownershipLabel(garage.ownership, copy))
      + detailRow(copy.garageId, garage.garageId)
      + detailRow(copy.cityId, garage.cityToken || copy.notAvailable)
      + detailRow(copy.hq, garage.isHeadquarters ? copy.hq : copy.regular)
      + detailRow(copy.currentSize, sizeLabel(garage.size, copy))
      + detailRow(copy.capacity, slotCountLabel(garage.vehicleSlotCount, copy))
      + detailRow(copy.occupiedSlots, garage.occupiedSlots)
      + detailRow(copy.availableSlots, garage.availableSlots)
      + detailRow(copy.assignedDrivers, formatCopy(copy.occupancyValue, {
        occupied: garage.assignedDriverCount,
        capacity: garage.driverSlotCount,
      }))
      + detailRow(copy.aiDriverPool, formatDriverPoolAvailability(state.driverPool, copy))
      + detailRow(copy.writeStatus, writeStatus)
      + detailRow(
        copy.backupStatus,
        lastResult
          ? lastResult.backupCreated ? copy.backupCreated : copy.backupNotCreated
          : copy.noOperationResult,
      )
      + detailRow(
        copy.verificationStatus,
        lastResult ? lastResult.verified ? copy.verified : copy.notVerified : copy.noOperationResult,
      )
      + detailRow(copy.backendStatus, garage.status ?? copy.notAvailable)
      + detailRow(copy.productivity, garage.productivity ?? copy.notAvailable)
      + "</dl></section>"
      + driverPreviewSection
      + "<section class='garage-detail-section'><h3>"
      + escapeHtml(copy.assignments)
      + "</h3><div class='garage-assignment-list'>"
      + assignmentSection(
        formatCopy(copy.driversCount, { count: driverIds.length }),
        driverIds,
        copy.noDrivers,
      )
      + assignmentSection(
        formatCopy(copy.trucksCount, { count: truckIds.length }),
        truckIds,
        copy.noTrucks,
      )
      + assignmentSection(
        formatCopy(copy.trailersCount, { count: (garage.trailerIds || []).length }),
        garage.trailerIds || [],
        copy.noTrailers,
      )
      + "</div></section><details class='garage-detail-section garage-warning-details'><summary>"
      + escapeHtml(copy.warnings)
      + "</summary>"
      + warningItems
      + "</details><section class='garage-detail-section garage-action-status'><h3>"
      + escapeHtml(copy.availableActions)
      + "</h3>"
      + actionBlockedMessage
      + "</section>";
    let actionMarkup = "";
    const mutationDisabled = readOnly || blocked || state.mutationPending;
    if (garage.ownership === "not_owned") {
      actionMarkup += "<button type='button' data-garage-detail-operation='purchase'"
        + (mutationDisabled ? " disabled" : "")
        + ">"
        + escapeHtml(copy.purchase)
        + "</button>";
    }
    if (garage.ownership === "owned" && garage.size === "small") {
      actionMarkup += "<button type='button' data-garage-detail-operation='upgrade'"
        + (mutationDisabled ? " disabled" : "")
        + ">"
        + escapeHtml(copy.upgrade)
        + "</button>";
    }
    if (garage.ownership === "owned" && garage.size === "large") {
      actionMarkup += "<button type='button' data-garage-detail-operation='downgrade'"
        + (readOnly || !canDowngradeGarage(garage) || state.mutationPending ? " disabled" : "")
        + ">"
        + escapeHtml(copy.downgrade)
        + "</button>";
    }
    if (garage.ownership === "owned") {
      actionMarkup += "<button type='button' data-garage-detail-operation='assign-resources'"
        + (mutationDisabled ? " disabled" : "")
        + ">"
        + escapeHtml(copy.assignResources)
        + "</button>";
    }
    if (garage.ownership === "owned" && !garage.isHeadquarters) {
      actionMarkup += "<button type='button' data-garage-detail-operation='headquarters'"
        + (mutationDisabled ? " disabled" : "")
        + ">"
        + escapeHtml(copy.setHeadquarters)
        + "</button>";
      actionMarkup += "<button type='button' data-garage-detail-operation='relinquish'"
        + (readOnly || !canRelinquishGarage(garage) || state.mutationPending ? " disabled" : "")
        + ">"
        + escapeHtml(copy.relinquish)
        + "</button>";
    }
    const footer = "<button type='button' class='button-secondary' data-garage-detail-close>"
      + escapeHtml(copy.close)
      + "</button><div class='garage-modal-primary-actions'><button type='button' "
      + "class='button-secondary' data-garage-detail-refresh>"
      + escapeHtml(copy.refresh)
      + "</button>"
      + actionMarkup
      + "</div>";
    const focusTarget = returnFocus || (() => focusGarageAction(garage.garageId, "details"));
    const modal = createModal(root, {
      title: copy.detailTitle,
      subtitle: garageCity(garage, copy),
      bodyMarkup: body,
      footerMarkup: footer,
      copy,
      className: "garage-detail-modal",
      returnFocus: focusTarget,
    });
    modal.overlay.querySelector("[data-garage-detail-close]")?.addEventListener(
      "click",
      modal.close,
    );
    modal.overlay.querySelector("[data-garage-detail-refresh]")?.addEventListener(
      "click",
      async () => {
        modal.close({ restoreFocus: false });
        await loadGarages({ toastOnError: true, preserveOnError: true });
        focusGarageAction(garage.garageId, "details");
      },
    );
    const reopenDetails = () => openDetails(garage.garageId, { returnFocus: focusTarget });
    const openDriverManager = (mode = "manual") => {
      modal.close({ restoreFocus: false });
      openDriverAssignmentDialog(garage.garageId, reopenDetails, { mode });
    };
    modal.overlay.querySelector("[data-garage-detail-driver-manager]")?.addEventListener(
      "click",
      () => openDriverManager("manual"),
    );
    modal.overlay.querySelector("[data-garage-detail-assign-random-drivers]")?.addEventListener(
      "click",
      () => openDriverManager("random"),
    );
    modal.overlay.querySelectorAll("[data-garage-driver-slot-assign]").forEach((button) => {
      button.addEventListener("click", () => openDriverManager("manual"));
    });
    modal.overlay.querySelectorAll("[data-garage-detail-operation]").forEach((button) => {
      button.addEventListener("click", () => {
        const operation = button.dataset.garageDetailOperation;
        modal.close({ restoreFocus: false });
        if (operation === "assign-resources") {
          openAssignmentDialog(garage.garageId, reopenDetails);
          return;
        }
        openActionDialog(
          garage.garageId,
          operation,
          reopenDetails,
        );
      });
    });
    if (focusActions) {
      modal.overlay.querySelector("[data-garage-detail-operation]:not([disabled])")?.focus();
    }
  }

  function openSellEmptyDialog() {
    if (!state.result || state.result.game !== "ets2" || state.mutationPending) return;
    const relinquishCount = allGarages().filter(canRelinquishGarage).length;
    const configuration = {
      failureTitle: copy.relinquishEmptyFailureTitle,
      command: "relinquish_empty_garages",
    };
    const body = "<section class='garage-dialog-effects'><h3>"
      + escapeHtml(copy.effects)
      + "</h3><dl class='garage-confirm-list'>"
      + detailRow(
        copy.desiredChange,
        formatCopy(copy.relinquishEmptyChange, { count: relinquishCount }),
      )
      + detailRow(copy.cost, copy.costNone)
      + detailRow(copy.backup, copy.backupAutomatic)
      + "</dl><p>"
      + escapeHtml(copy.relinquishEmptyEffect)
      + "</p></section><p class='garage-save-warning'>"
      + escapeHtml(copy.saveWarning)
      + "</p><div class='garage-mutation-error' data-garage-mutation-error hidden></div>";
    const footer = "<button type='button' class='button-secondary' data-garage-dialog-cancel>"
      + escapeHtml(copy.cancel)
      + "</button><div class='garage-modal-primary-actions'><button type='button' "
      + "data-garage-dialog-apply>"
      + escapeHtml(copy.relinquishEmpty)
      + "</button></div>";
    const modal = createModal(root, {
      title: copy.relinquishEmptyDialogTitle,
      bodyMarkup: body,
      footerMarkup: footer,
      copy,
      className: "garage-action-modal",
      returnFocus: () => sellEmptyButton.focus(),
    });
    const applyButton = modal.overlay.querySelector("[data-garage-dialog-apply]");
    const cancelButton = modal.overlay.querySelector("[data-garage-dialog-cancel]");
    const errorElement = modal.overlay.querySelector("[data-garage-mutation-error]");
    cancelButton.addEventListener("click", modal.close);
    applyButton.addEventListener("click", async () => {
      if (state.mutationPending) return;
      state.mutationPending = true;
      state.bulkMutationPending = true;
      state.bulkMutationOperation = "relinquish_empty";
      applyButton.disabled = true;
      applyButton.innerHTML = "<span class='garage-spinner' aria-hidden='true'></span>"
        + escapeHtml(copy.relinquishingEmpty);
      errorElement.hidden = true;
      render();
      try {
        const request = { expectedSaveHash: state.result.saveHash };
        const result = await window.invoke("relinquish_empty_garages", { request });
        validateBulkMutationResult(result, "relinquish_empty");
        state.lastMutationResult = result;
        state.highlightedGarageId = null;
        modal.close({ restoreFocus: false });
        await loadGarages({
          toastOnError: true,
          expectedSaveHash: result.saveHash,
          preserveOnError: true,
        });
        const successKey = result.relinquishedCount > 0
          ? "garage_manager.success.relinquish_empty"
          : "garage_manager.success.relinquish_empty_none";
        window.showToast(successKey, { count: result.relinquishedCount }, "success");
        await Promise.allSettled([
          window.loadAllTrucks?.(),
          window.loadAllTrailers?.(),
          window.loadProfileData?.(),
          window.refreshOperationalOverview?.(),
        ]);
        sellEmptyButton.focus();
      } catch (error) {
        console.error("Garage empty batch sale failed:", error);
        await showMutationError(errorElement, error, configuration);
      } finally {
        state.bulkMutationPending = false;
        state.bulkMutationOperation = null;
        state.mutationPending = false;
        if (root.isConnected) render();
        if (applyButton.isConnected) {
          applyButton.disabled = false;
          applyButton.textContent = copy.relinquishEmpty;
        }
      }
    });
  }
  function openBuyAllDialog() {
    if (!state.result || state.result.game !== "ets2" || state.mutationPending) return;
    const purchaseCount = allGarages()
      .filter((garage) => garage.ownership === "not_owned")
      .length;
    const configuration = {
      failureTitle: copy.purchaseAllFailureTitle,
      command: "buy_all_garages",
    };
    const body = "<section class='garage-dialog-effects'><h3>"
      + escapeHtml(copy.effects)
      + "</h3><dl class='garage-confirm-list'>"
      + detailRow(
        copy.desiredChange,
        formatCopy(copy.purchaseAllChange, { count: purchaseCount }),
      )
      + detailRow(copy.cost, copy.costNone)
      + detailRow(copy.backup, copy.backupAutomatic)
      + "</dl><p>"
      + escapeHtml(copy.purchaseNoExtras)
      + "</p></section><p class='garage-save-warning'>"
      + escapeHtml(copy.saveWarning)
      + "</p><div class='garage-mutation-error' data-garage-mutation-error hidden></div>";
    const footer = "<button type='button' class='button-secondary' data-garage-dialog-cancel>"
      + escapeHtml(copy.cancel)
      + "</button><div class='garage-modal-primary-actions'><button type='button' "
      + "data-garage-dialog-apply>"
      + escapeHtml(copy.purchaseAll)
      + "</button></div>";
    const modal = createModal(root, {
      title: copy.purchaseAllDialogTitle,
      bodyMarkup: body,
      footerMarkup: footer,
      copy,
      className: "garage-action-modal",
      returnFocus: () => buyAllButton.focus(),
    });
    const applyButton = modal.overlay.querySelector("[data-garage-dialog-apply]");
    const cancelButton = modal.overlay.querySelector("[data-garage-dialog-cancel]");
    const errorElement = modal.overlay.querySelector("[data-garage-mutation-error]");
    cancelButton.addEventListener("click", modal.close);
    applyButton.addEventListener("click", async () => {
      if (state.mutationPending) return;
      state.mutationPending = true;
      state.bulkMutationPending = true;
      state.bulkMutationOperation = "purchase_all";
      applyButton.disabled = true;
      applyButton.innerHTML = "<span class='garage-spinner' aria-hidden='true'></span>"
        + escapeHtml(copy.purchasingAll);
      errorElement.hidden = true;
      render();
      try {
        const request = { expectedSaveHash: state.result.saveHash };
        const result = await window.invoke("buy_all_garages", { request });
        validateBulkMutationResult(result, "purchase_all");
        state.lastMutationResult = result;
        state.highlightedGarageId = null;
        modal.close({ restoreFocus: false });
        await loadGarages({
          toastOnError: true,
          expectedSaveHash: result.saveHash,
          preserveOnError: true,
        });
        const successKey = result.purchasedCount > 0
          ? "garage_manager.success.purchase_all"
          : "garage_manager.success.purchase_all_none";
        window.showToast(successKey, { count: result.purchasedCount }, "success");
        await Promise.allSettled([
          window.loadAllTrucks?.(),
          window.loadAllTrailers?.(),
          window.loadProfileData?.(),
          window.refreshOperationalOverview?.(),
        ]);
        buyAllButton.focus();
      } catch (error) {
        console.error("Garage batch purchase failed:", error);
        await showMutationError(errorElement, error, configuration);
      } finally {
        state.bulkMutationPending = false;
        state.bulkMutationOperation = null;
        state.mutationPending = false;
        if (root.isConnected) render();
        if (applyButton.isConnected) {
          applyButton.disabled = false;
          applyButton.textContent = copy.purchaseAll;
        }
      }
    });
  }

  function operationConfiguration(garage, operation) {
    const currentSize = sizeLabel(garage.size, copy)
      + " · "
      + slotCountLabel(garage.vehicleSlotCount, copy);
    if (operation === "purchase" && garage.ownership === "not_owned") {
      return {
        title: copy.purchaseDialogTitle,
        currentState: ownershipLabel(garage.ownership, copy),
        futureState: copy.purchaseChange,
        effect: copy.purchaseNoExtras,
        buttonLabel: copy.purchase,
        loadingLabel: copy.purchasing,
        failureTitle: copy.purchaseFailureTitle,
        command: "purchase_garage",
        request: {},
      };
    }
    if (operation === "upgrade" && garage.ownership === "owned" && garage.size === "small") {
      return {
        title: copy.upgradeDialogTitle,
        currentState: currentSize,
        futureState: copy.updateLargeChange,
        effect: copy.upgradeChange,
        buttonLabel: copy.upgrade,
        loadingLabel: copy.upgrading,
        failureTitle: copy.upgradeFailureTitle,
        command: "upgrade_owned_garage",
        request: {},
      };
    }
    if (operation === "relinquish" && canRelinquishGarage(garage)) {
      return {
        title: copy.relinquishDialogTitle,
        currentState: ownershipLabel(garage.ownership, copy) + " · " + currentSize,
        futureState: copy.relinquishChange,
        effect: copy.relinquishEffect,
        buttonLabel: copy.relinquish,
        loadingLabel: copy.relinquishing,
        failureTitle: copy.relinquishFailureTitle,
        command: "relinquish_garage_ownership",
        request: {},
      };
    }
    if (operation === "downgrade"
      && garage.ownership === "owned"
      && canDowngradeGarage(garage)) {
      return {
        title: copy.downgradeDialogTitle,
        currentState: currentSize,
        futureState: copy.updateSmallChange,
        effect: formatCopy(copy.occupancyValue, {
          occupied: garage.occupiedSlots,
          capacity: garage.vehicleSlotCount,
        }),
        buttonLabel: copy.downgrade,
        loadingLabel: copy.downgrading,
        failureTitle: copy.downgradeFailureTitle,
        command: "update_garage",
        request: {
          targetSize: "small",
          setAsHeadquarters: false,
        },
      };
    }
    if (operation === "headquarters"
      && garage.ownership === "owned"
      && !garage.isHeadquarters) {
      return {
        title: copy.headquartersDialogTitle,
        currentState: copy.regular,
        futureState: formatCopy(copy.newHeadquarters, {
          city: garageCity(garage, copy),
        }),
        effect: copy.updateHqChange,
        buttonLabel: copy.setHeadquarters,
        loadingLabel: copy.changingHeadquarters,
        failureTitle: copy.headquartersFailureTitle,
        command: "update_garage",
        request: {
          targetSize: null,
          setAsHeadquarters: true,
        },
      };
    }
    return null;
  }

  async function showMutationError(errorElement, error, configuration) {
    const key = errorTranslationKey(error);
    const localized = await window.t(key);
    const hint = errorCode(error).includes("save_changed_since_load")
      ? "<p>" + escapeHtml(copy.saveChangedHint) + "</p>"
      : "";
    if (errorElement.isConnected) {
      errorElement.hidden = false;
      errorElement.innerHTML = "<h3>"
        + escapeHtml(configuration.failureTitle)
        + "</h3><p>"
        + escapeHtml(localized)
        + "</p>"
        + hint
        + "<details><summary>"
        + escapeHtml(copy.technicalDetails)
        + "</summary><dl class='garage-error-details'>"
        + detailRow(copy.errorCode, errorCode(error).split(":")[0] || copy.unknown)
        + detailRow(copy.backendMessage, technicalMessage(error, copy))
        + detailRow(copy.command, configuration.command)
        + detailRow(copy.verificationStatus, copy.verificationNotCompleted)
        + "</dl></details>";
    }
    window.showToast(key, {}, "error");
  }

  function validateDriverAssignmentResult(result, garageId, expectedCount = null) {
    if (!result?.verified
      || result.garageId !== garageId
      || !result.saveHash
      || !Array.isArray(result.assignedDriverIds)
      || result.assignedDriverIds.length !== Number(result.assignedCount || 0)
      || (Number.isInteger(expectedCount) && Number(result.assignedCount || 0) !== expectedCount)) {
      throw new Error("garage_size_change_not_verified:driver_assignment_response");
    }
  }

  function openDriverAssignmentDialog(garageId, returnFocus = null, options = {}) {
    let garage = findGarage(garageId);
    if (!garage || state.result?.game !== "ets2") return;

    const focusTarget = returnFocus || (() => focusGarageAction(garage.garageId, "details"));
    let selectedCount = driverAssignmentLimit(garage, state.driverPool);
    let isAssigningDrivers = false;
    let successResult = null;
    let driverAssignmentError = null;
    let searchQuery = "";

    const modal = createModal(root, {
      title: copy.driverManagerTitle,
      subtitle: garageCity(garage, copy),
      bodyMarkup: "<div data-garage-driver-manager-body></div>",
      footerMarkup: "<button type='button' class='button-secondary' data-garage-driver-action='cancel'>"
        + escapeHtml(copy.cancel)
        + "</button>",
      copy,
      className: "garage-driver-manager-modal",
      returnFocus: focusTarget,
      canClose: () => !isAssigningDrivers,
    });
    const bodyElement = modal.overlay.querySelector("[data-garage-driver-manager-body]");
    const footerElement = modal.overlay.querySelector(".garage-modal-footer");
    const closeButton = modal.overlay.querySelector("[data-garage-modal-close]");

    function currentDriverIds(currentGarage) {
      return (currentGarage?.slots || []).map((slot) => slot.driverId).filter(Boolean);
    }

    function renderDriverCards(entries, canAssignManual) {
      const normalizedSearch = searchQuery.trim().toLowerCase();
      const filtered = normalizedSearch
        ? entries.filter((entry) => entry.searchText.includes(normalizedSearch))
        : entries;
      if (!filtered.length) {
        return "<p class='garage-reference-empty'>" + escapeHtml(copy.noDriverMatches) + "</p>";
      }
      return "<div class='garage-driver-pool-list'>"
        + filtered.map((entry) => "<article class='garage-driver-pool-card"
          + (entry.isAvailable ? " is-available" : " is-disabled")
          + "'><div><strong>"
          + escapeHtml(copy.aiDriver)
          + "</strong><code>"
          + escapeHtml(entry.driverId)
          + "</code><span>"
          + escapeHtml(entry.status)
          + "</span></div><button type='button' data-garage-driver-action='assign-manual' data-driver-id='"
          + escapeHtml(entry.driverId)
          + "'"
          + (!entry.isAvailable || !canAssignManual || isAssigningDrivers ? " disabled" : "")
          + ">"
          + escapeHtml(copy.assignDriver)
          + "</button></article>").join("")
        + "</div>";
    }

    function renderDriverManager() {
      const currentGarage = findGarage(garageId) || garage;
      garage = currentGarage;
      const owned = currentGarage.ownership === "owned";
      const blockedNow = garageIsBlocked(currentGarage);
      const capacity = Math.max(0, Number(currentGarage.driverSlotCount || 0));
      const assigned = Math.max(0, Number(currentGarage.assignedDriverCount || 0));
      const freeSlots = freeDriverSlotCount(currentGarage);
      const availableDrivers = Math.max(0, Number(state.driverPool?.availableDriverCount || 0));
      const maximum = owned && !blockedNow ? Math.min(freeSlots, availableDrivers) : 0;
      selectedCount = clampDriverSelection(selectedCount, maximum);
      const progress = capacity > 0 ? Math.min(100, Math.round((assigned / capacity) * 100)) : 0;
      const driverIds = currentDriverIds(currentGarage);
      const driverItems = driverIds.length
        ? "<ul class='garage-driver-current-list'>"
          + driverIds.map((driverId) => "<li><code>" + escapeHtml(driverId) + "</code></li>").join("")
          + "</ul>"
        : "<p class='garage-reference-empty'>" + escapeHtml(copy.noDrivers) + "</p>";
      const noTruckCount = driversWithoutTruckCount(currentGarage);
      const noTruckNotice = noTruckCount > 0
        ? "<p class='garage-driver-note'>"
          + escapeHtml(formatCopy(copy.driverNoTruckWarning, { count: noTruckCount }))
          + "</p>"
        : "";
      const statusMessage = !owned
        ? copy.driverGarageNotOwned
        : blockedNow
          ? copy.mutationBlocked
          : freeSlots <= 0
          ? copy.garageFullyStaffedMessage
          : availableDrivers <= 0
            ? copy.driverPoolEmptyMessage
            : "";
      const statusTitle = !owned
        ? copy.driverGarageNotOwned
        : blockedNow
          ? copy.writeBlocked
          : freeSlots <= 0
          ? copy.garageFullyStaffed
          : availableDrivers <= 0
            ? copy.driverPoolEmpty
            : "";
      const statusNote = statusMessage
        ? "<section class='garage-driver-state-note'><strong>"
          + escapeHtml(statusTitle)
          + "</strong><span>"
          + escapeHtml(statusMessage)
          + "</span></section>"
        : "";
      const canAssignManual = owned && !blockedNow && freeSlots > 0 && !state.mutationPending;
      const entries = driverPoolEntries(state.driverPool, allGarages(), copy);
      const poolMarkup = "<section class='garage-driver-panel garage-driver-pool-panel'><div class='garage-driver-pool-head'><h3>"
        + escapeHtml(copy.selectAiDriver)
        + "</h3><span>"
        + escapeHtml(formatCopy(copy.availableDriversCount, { count: availableDrivers }))
        + "</span></div><input type='search' class='garage-driver-search' data-garage-driver-search aria-label='"
        + escapeHtml(copy.searchDriversPlaceholder)
        + "' placeholder='"
        + escapeHtml(copy.searchDriversPlaceholder)
        + "' value='"
        + escapeHtml(searchQuery)
        + "'>"
        + renderDriverCards(entries, canAssignManual)
        + "</section>";
      const randomMarkup = maximum > 0 && !successResult
        ? "<section class='garage-driver-panel'><h3>"
          + escapeHtml(copy.assignRandomDrivers)
          + "</h3><div class='garage-driver-stepper' role='group' aria-label='"
          + escapeHtml(copy.howManyDrivers)
          + "'><button type='button' data-garage-driver-action='decrease' aria-label='"
          + escapeHtml(copy.decreaseDriverCount)
          + "'"
          + (selectedCount <= 1 || isAssigningDrivers ? " disabled" : "")
          + "><span aria-hidden='true'>-</span></button><strong>"
          + escapeHtml(selectedCount)
          + "</strong><button type='button' data-garage-driver-action='increase' aria-label='"
          + escapeHtml(copy.increaseDriverCount)
          + "'"
          + (selectedCount >= maximum || isAssigningDrivers ? " disabled" : "")
          + "><span aria-hidden='true'>+</span></button></div><p class='garage-reference-empty'>"
          + escapeHtml(formatCopy(copy.maximumAvailableDrivers, { count: maximum }))
          + "</p><button type='button' class='button-secondary' data-garage-driver-action='fill'"
          + (isAssigningDrivers ? " disabled" : "")
          + ">"
          + escapeHtml(copy.fillAllDriverPositions)
          + "</button><button type='button' data-garage-driver-action='assign-random'"
          + (isAssigningDrivers ? " disabled" : "")
          + ">"
          + (isAssigningDrivers
            ? "<span class='garage-spinner' aria-hidden='true'></span>" + escapeHtml(copy.assigningDrivers)
            : escapeHtml(driverAssignButtonLabel(selectedCount, copy)))
          + "</button></section>"
        : "";
      const successMarkup = successResult
        ? "<section class='garage-driver-feedback is-success'><strong>"
          + escapeHtml(formatCopy(copy.driverAssignSuccess, { count: successResult.assignedCount }))
          + "</strong><ul class='garage-driver-current-list'>"
          + successResult.assignedDriverIds.map((driverId) => "<li><code>" + escapeHtml(driverId) + "</code></li>").join("")
          + "</ul></section>"
        : "";
      const errorMarkup = driverAssignmentError
        ? "<section class='garage-mutation-error'><h3>"
          + escapeHtml(driverAssignmentError.failureTitle)
          + "</h3><p>"
          + escapeHtml(driverAssignmentError.localized)
          + "</p><details><summary>"
          + escapeHtml(copy.technicalDetails)
          + "</summary><dl class='garage-error-details'>"
          + detailRow(copy.errorCode, driverAssignmentError.code)
          + detailRow(copy.backendMessage, driverAssignmentError.detail)
          + detailRow(copy.command, driverAssignmentError.command)
          + detailRow(copy.verificationStatus, copy.verificationNotCompleted)
          + "</dl></details></section>"
        : "";

      bodyElement.innerHTML = successMarkup
        + errorMarkup
        + "<section class='garage-driver-panel'><h3>"
        + escapeHtml(copy.driverPositions)
        + "</h3><div class='garage-driver-capacity'><strong>"
        + escapeHtml(formatCopy(copy.driverPositionStatus, { assigned, capacity }))
        + "</strong><span>"
        + escapeHtml(formatCopy(copy.freeDriverPositionsStatus, { count: freeSlots }))
        + "</span></div><div class='garage-driver-progress' aria-hidden='true'><span style='width: "
        + progress
        + "%'></span></div><dl class='garage-confirm-list'>"
        + detailRow(copy.assignedDrivers, assigned)
        + detailRow(copy.freeDriverPositions, freeSlots)
        + detailRow(copy.availableAiDrivers, availableDrivers)
        + "</dl>"
        + noTruckNotice
        + statusNote
        + "</section>"
        + (options.mode === "random" ? randomMarkup + poolMarkup : poolMarkup + randomMarkup)
        + "<section class='garage-driver-panel'><h3>"
        + escapeHtml(copy.currentDrivers)
        + "</h3>"
        + driverItems
        + "</section>";

      footerElement.innerHTML = "<button type='button' class='button-secondary' data-garage-driver-action='cancel'"
        + (isAssigningDrivers ? " disabled" : "")
        + ">"
        + escapeHtml(successResult ? copy.done : copy.cancel)
        + "</button>";
      closeButton.disabled = isAssigningDrivers;
    }

    async function applyDriverAssignment(command, request, expectedCount) {
      if (isAssigningDrivers || state.mutationPending) return;
      isAssigningDrivers = true;
      state.mutationPending = true;
      driverAssignmentError = null;
      successResult = null;
      renderDriverManager();
      render();
      try {
        const result = await window.invoke(command, { request });
        validateDriverAssignmentResult(result, garageId, expectedCount);
        state.lastMutationResult = null;
        state.lastDriverAssignmentResult = result;
        state.highlightedGarageId = result.garageId;
        state.selectedGarageId = result.garageId;
        persistViewState();
        await loadGarages({
          toastOnError: true,
          expectedSaveHash: result.saveHash,
          preserveOnError: true,
        });
        garage = findGarage(result.garageId) || garage;
        successResult = result;
        selectedCount = driverAssignmentLimit(garage, state.driverPool);
        window.showToast(
          "garage_manager.success.assign_drivers",
          {
            city: garageCity(garage, copy),
            count: result.assignedCount,
            drivers: result.assignedDriverIds.join(", "),
          },
          "success",
        );
        await Promise.allSettled([
          window.loadAllTrucks?.(),
          window.loadAllTrailers?.(),
          window.loadProfileData?.(),
          window.refreshOperationalOverview?.(),
        ]);
      } catch (error) {
        console.error("AI driver assignment failed:", error);
        const key = errorTranslationKey(error);
        driverAssignmentError = {
          failureTitle: copy.assignDriversFailureTitle,
          localized: await window.t(key),
          code: errorCode(error).split(":")[0] || copy.unknown,
          detail: technicalMessage(error, copy),
          command,
        };
        window.showToast(key, {}, "error");
      } finally {
        isAssigningDrivers = false;
        state.mutationPending = false;
        if (root.isConnected) render();
        if (bodyElement.isConnected) renderDriverManager();
      }
    }

    async function assignRandomDrivers() {
      if (selectedCount <= 0) return;
      const expectedCount = selectedCount;
      await applyDriverAssignment("assign_random_ai_drivers_to_garage", {
        garageId,
        expectedSaveHash: state.result.saveHash,
        count: expectedCount,
      }, expectedCount);
    }

    async function assignManualDriver(driverRef) {
      if (!driverRef) return;
      await applyDriverAssignment("assign_ai_driver_to_garage", {
        garageId,
        expectedSaveHash: state.result.saveHash,
        driverRef,
      }, 1);
    }

    modal.overlay.addEventListener("input", (event) => {
      const input = event.target instanceof HTMLInputElement
        ? event.target.closest("[data-garage-driver-search]")
        : null;
      if (!input) return;
      searchQuery = input.value;
      driverAssignmentError = null;
      renderDriverManager();
      const nextInput = modal.overlay.querySelector("[data-garage-driver-search]");
      nextInput?.focus();
      nextInput?.setSelectionRange(searchQuery.length, searchQuery.length);
    });

    modal.overlay.addEventListener("click", (event) => {
      const button = event.target instanceof Element
        ? event.target.closest("[data-garage-driver-action]")
        : null;
      if (!button || !modal.overlay.contains(button) || button.disabled) return;
      const action = button.dataset.garageDriverAction;
      if (action === "cancel") {
        modal.close();
      } else if (action === "decrease") {
        selectedCount = clampDriverSelection(selectedCount - 1, driverAssignmentLimit(garage, state.driverPool));
        driverAssignmentError = null;
        renderDriverManager();
      } else if (action === "increase") {
        selectedCount = clampDriverSelection(selectedCount + 1, driverAssignmentLimit(garage, state.driverPool));
        driverAssignmentError = null;
        renderDriverManager();
      } else if (action === "fill") {
        selectedCount = driverAssignmentLimit(garage, state.driverPool);
        driverAssignmentError = null;
        renderDriverManager();
      } else if (action === "assign-random") {
        void assignRandomDrivers();
      } else if (action === "assign-manual") {
        void assignManualDriver(button.dataset.driverId);
      }
    });

    renderDriverManager();
  }  function openAssignmentDialog(garageId, returnFocus = null) {
    const garage = findGarage(garageId);
    if (!garage || state.result?.game !== "ets2" || garageIsBlocked(garage)) return;
    const configuration = {
      failureTitle: copy.assignResourcesFailureTitle,
      command: "assign_random_garage_resources",
    };
    const currentState = sizeLabel(garage.size, copy)
      + " / "
      + formatCopy(copy.occupancyValue, {
        occupied: garage.occupiedSlots,
        capacity: garage.vehicleSlotCount,
      });
    const comparison = "<div class='garage-state-comparison'><article><span>"
      + escapeHtml(copy.currentState)
      + "</span><strong>"
      + escapeHtml(currentState)
      + "</strong></article><article class='is-future'><span>"
      + escapeHtml(copy.futureState)
      + "</span><strong>"
      + escapeHtml(copy.assignResourcesChange)
      + "</strong></article></div>";
    const body = comparison
      + "<section class='garage-dialog-effects'><h3>"
      + escapeHtml(copy.effects)
      + "</h3><p>"
      + escapeHtml(copy.assignResourcesEffect)
      + "</p><label class='garage-checkbox'><input type='checkbox' data-garage-random-driver><span>"
      + escapeHtml(copy.randomDriver)
      + "</span></label><label class='garage-checkbox'><input type='checkbox' data-garage-random-truck><span>"
      + escapeHtml(copy.randomTruck)
      + "</span></label><dl class='garage-confirm-list'>"
      + detailRow(copy.cost, copy.costNone)
      + detailRow(copy.backup, copy.backupAutomatic)
      + "</dl></section><p class='garage-save-warning'>"
      + escapeHtml(copy.saveWarning)
      + "</p><div class='garage-mutation-error' data-garage-mutation-error hidden></div>";
    const footer = "<button type='button' class='button-secondary' data-garage-dialog-cancel>"
      + escapeHtml(copy.cancel)
      + "</button><div class='garage-modal-primary-actions'><button type='button' "
      + "data-garage-dialog-apply>"
      + escapeHtml(copy.assignResources)
      + "</button></div>";
    const focusTarget = returnFocus || (() => focusGarageAction(garage.garageId, "details"));
    const modal = createModal(root, {
      title: copy.assignResourcesDialogTitle,
      subtitle: garageCity(garage, copy),
      bodyMarkup: body,
      footerMarkup: footer,
      copy,
      className: "garage-action-modal",
      returnFocus: focusTarget,
    });
    const applyButton = modal.overlay.querySelector("[data-garage-dialog-apply]");
    const cancelButton = modal.overlay.querySelector("[data-garage-dialog-cancel]");
    const errorElement = modal.overlay.querySelector("[data-garage-mutation-error]");
    cancelButton.addEventListener("click", modal.close);
    applyButton.addEventListener("click", async () => {
      if (state.mutationPending) return;
      const assignRandomDriver = Boolean(modal.overlay.querySelector("[data-garage-random-driver]")?.checked);
      const assignRandomTruck = Boolean(modal.overlay.querySelector("[data-garage-random-truck]")?.checked);
      errorElement.hidden = true;
      if (!assignRandomDriver && !assignRandomTruck) {
        await showMutationError(errorElement, new Error("garage_assignment_empty"), configuration);
        return;
      }
      state.mutationPending = true;
      applyButton.disabled = true;
      applyButton.innerHTML = "<span class='garage-spinner' aria-hidden='true'></span>"
        + escapeHtml(copy.assigningResources);
      render();
      try {
        const request = {
          garageId,
          expectedSaveHash: state.result.saveHash,
          assignRandomDriver,
          assignRandomTruck,
        };
        const result = await window.invoke("assign_random_garage_resources", { request });
        validateMutationResult(result);
        state.lastMutationResult = result;
        state.highlightedGarageId = result.garageId;
        state.selectedGarageId = result.garageId;
        persistViewState();
        modal.close({ restoreFocus: false });
        await loadGarages({
          toastOnError: true,
          expectedSaveHash: result.saveHash,
          preserveOnError: true,
        });
        window.showToast(
          mutationSuccessKey(result),
          { city: garageCity(result.updatedState, copy) },
          "success",
        );
        await Promise.allSettled([
          window.loadAllTrucks?.(),
          window.loadAllTrailers?.(),
          window.loadProfileData?.(),
          window.refreshOperationalOverview?.(),
        ]);
        focusGarageAction(result.garageId, "details");
      } catch (error) {
        console.error("Garage random assignment failed:", error);
        await showMutationError(errorElement, error, configuration);
      } finally {
        state.mutationPending = false;
        if (root.isConnected) render();
        if (applyButton.isConnected) {
          applyButton.disabled = false;
          applyButton.textContent = copy.assignResources;
        }
      }
    });
  }
  function openActionDialog(garageId, operation, returnFocus = null) {
    const garage = findGarage(garageId);
    if (!garage || state.result?.game !== "ets2" || garageIsBlocked(garage)) return;
    const configuration = operationConfiguration(garage, operation);
    if (!configuration) return;
    const comparison = "<div class='garage-state-comparison'><article><span>"
      + escapeHtml(copy.currentState)
      + "</span><strong>"
      + escapeHtml(configuration.currentState)
      + "</strong></article><article class='is-future'><span>"
      + escapeHtml(copy.futureState)
      + "</span><strong>"
      + escapeHtml(configuration.futureState)
      + "</strong></article></div>";
    const optionalExtensions = "";
    const downgradeNotice = operation === "downgrade"
      ? "<p class='garage-downgrade-warning'>" + escapeHtml(copy.downgradeWarning) + "</p>"
      : "";
    const body = comparison
      + "<section class='garage-dialog-effects'><h3>"
      + escapeHtml(copy.effects)
      + "</h3><p>"
      + escapeHtml(configuration.effect)
      + "</p><dl class='garage-confirm-list'>"
      + detailRow(copy.cost, copy.costNone)
      + detailRow(copy.backup, copy.backupAutomatic)
      + "</dl></section><p class='garage-save-warning'>"
      + escapeHtml(copy.saveWarning)
      + "</p>"
      + downgradeNotice
      + optionalExtensions
      + "<div class='garage-mutation-error' data-garage-mutation-error hidden></div>";
    const footer = "<button type='button' class='button-secondary' data-garage-dialog-cancel>"
      + escapeHtml(copy.cancel)
      + "</button><div class='garage-modal-primary-actions'><button type='button' "
      + "data-garage-dialog-apply>"
      + escapeHtml(configuration.buttonLabel)
      + "</button></div>";
    const focusTarget = returnFocus || (() => focusGarageAction(garage.garageId, "details"));
    const modal = createModal(root, {
      title: configuration.title,
      subtitle: garageCity(garage, copy),
      bodyMarkup: body,
      footerMarkup: footer,
      copy,
      className: "garage-action-modal",
      returnFocus: focusTarget,
    });
    const applyButton = modal.overlay.querySelector("[data-garage-dialog-apply]");
    const cancelButton = modal.overlay.querySelector("[data-garage-dialog-cancel]");
    const errorElement = modal.overlay.querySelector("[data-garage-mutation-error]");
    cancelButton.addEventListener("click", modal.close);
    applyButton.addEventListener("click", async () => {
      if (state.mutationPending) return;
      state.mutationPending = true;
      applyButton.disabled = true;
      applyButton.innerHTML = "<span class='garage-spinner' aria-hidden='true'></span>"
        + escapeHtml(configuration.loadingLabel);
      errorElement.hidden = true;
      render();
      try {
        const currentGarage = findGarage(garageId);
        const currentConfiguration = currentGarage
          ? operationConfiguration(currentGarage, operation)
          : null;
        if (!currentGarage || !currentConfiguration) {
          throw new Error("garage_not_found");
        }
        const request = {
          garageId,
          expectedSaveHash: state.result.saveHash,
          ...currentConfiguration.request,
        };
        const result = await window.invoke(currentConfiguration.command, { request });
        validateMutationResult(result);
        state.lastMutationResult = result;
        state.highlightedGarageId = result.garageId;
        state.selectedGarageId = result.garageId;
        persistViewState();
        modal.close({ restoreFocus: false });
        await loadGarages({
          toastOnError: true,
          expectedSaveHash: result.saveHash,
          preserveOnError: true,
        });
        const successKey = mutationSuccessKey(result);
        window.showToast(successKey, { city: garageCity(result.updatedState, copy) }, "success");
        await Promise.allSettled([
          window.loadAllTrucks?.(),
          window.loadAllTrailers?.(),
          window.loadProfileData?.(),
          window.refreshOperationalOverview?.(),
        ]);
        focusGarageAction(result.garageId, "details");
      } catch (error) {
        console.error("Garage mutation failed:", error);
        await showMutationError(errorElement, error, configuration);
      } finally {
        state.mutationPending = false;
        if (root.isConnected) render();
        if (applyButton.isConnected) {
          applyButton.disabled = false;
          applyButton.textContent = configuration.buttonLabel;
        }
      }
    });
  }

  root.addEventListener("input", (event) => {
    const search = event.target.dataset.garageSearch;
    if (search) {
      state[search] = event.target.value;
      persistViewState();
      renderFilterStatus();
      renderList();
    }
  });
  root.addEventListener("change", (event) => {
    const filter = event.target.dataset.garageFilter;
    if (!filter) return;
    state[filter] = event.target.value;
    persistViewState();
    renderFilterStatus();
    renderList();
  });
  root.addEventListener("click", async (event) => {
    if (event.target.closest("[data-garage-sell-empty]")) {
      openSellEmptyDialog();
      return;
    }
    if (event.target.closest("[data-garage-buy-all]")) {
      openBuyAllDialog();
      return;
    }
    if (event.target.closest("[data-garage-refresh], [data-garage-retry]")) {
      if (state.mutationPending) return;
      await loadGarages({ toastOnError: true, preserveOnError: Boolean(state.result) });
      return;
    }
    if (event.target.closest("[data-garage-filter-reset]")) {
      Object.assign(state, {
        citySearch: "",
        idSearch: "",
        ownership: "all",
        size: "all",
        hq: "all",
        occupancy: "all",
      });
      root.querySelectorAll("[data-garage-search]").forEach((input) => {
        input.value = "";
      });
      root.querySelectorAll("[data-garage-filter]").forEach((select) => {
        if (select.dataset.garageFilter !== "sort") select.value = "all";
      });
      persistViewState();
      renderFilterStatus();
      renderList();
      return;
    }
    const button = event.target.closest("[data-garage-action]");
    if (!button) return;
    const garage = findGarage(button.dataset.garageId);
    if (!garage) return;
    const action = button.dataset.garageAction;
    state.selectedGarageId = garage.garageId;
    persistViewState();
    renderList();
    const focusTarget = () => focusGarageAction(garage.garageId, action);
    if (action === "details") {
      openDetails(garage.garageId, { returnFocus: focusTarget });
      return;
    }
    if (action === "details-actions") {
      openDetails(garage.garageId, { focusActions: true, returnFocus: focusTarget });
      return;
    }
    if (state.mutationPending) return;
    if (action === "purchase") openActionDialog(garage.garageId, "purchase", focusTarget);
    if (action === "upgrade") openActionDialog(garage.garageId, "upgrade", focusTarget);
  });

  await loadGarages();
}
