export function activeGame() {
  return window.selectedGame === "ats" ? "ats" : "ets2";
}

export function currencyCode() {
  return activeGame() === "ats" ? "USD" : "EUR";
}

export function distanceUnit() {
  const preference = window.readSaveGameConfig?.use_miles;
  return (typeof preference === "boolean" ? preference : activeGame() === "ats") ? "mi" : "km";
}

export function displayDistance(kilometers) {
  return distanceUnit() === "mi" ? Number(kilometers) / 1.609344 : Number(kilometers);
}

export function storedDistance(value) {
  return distanceUnit() === "mi" ? Number(value) * 1.609344 : Number(value);
}

export function formatGameMoney(value, digits = 0) {
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: currencyCode(),
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(Number(value));
}
