const { invoke } = window.__TAURI__.core;

document.addEventListener("DOMContentLoaded", () => {
    const scanBtn = document.querySelector("#refreshBtn");
    const profileNameDisplay = document.querySelector("#profileNameDisplay");
    const profileDropdownList = document.querySelector("#profileDropdownList");
    const openProfileModalBtn = document.querySelector("#openProfileModal");
    const profileStatus = document.querySelector("#profile-status");

    const moneyDisplay = document.querySelector("#moneyShow");
    const xpDisplay = document.querySelector("#xpShow");

    const moneyBtn = document.querySelector("#save-money-btn");
    const levelBtn = document.querySelector("#save-level-btn");
    const editStatus = document.querySelector("#edit-status");

    let selectedProfilePath = null;

    // Dropdown toggle
    function toggleProfileDropdown() {
        profileDropdownList.classList.toggle("show");
    }

    document.addEventListener("click", (event) => {
        if (!event.target.closest(".profile-picker")) {
            profileDropdownList.classList.remove("show");
        }
    });

    openProfileModalBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        toggleProfileDropdown();
    });

    // Scan profiles
    scanBtn.addEventListener("click", async () => {
        profileStatus.textContent = "Scanning profiles...";
        profileDropdownList.innerHTML = "";

        const profiles = await invoke("find_ets2_profiles");

        profiles.forEach((p) => {
            if (!p.success) return;

            const item = document.createElement("div");
            item.classList.add("dropdown-item");
            item.textContent = `${p.name} (${p.path})`;
            item.dataset.path = p.path;

            item.addEventListener("click", () => {
                selectedProfilePath = p.path;
                profileNameDisplay.textContent = p.name;
                profileDropdownList.classList.remove("show");
                loadSelectedProfile();
            });

            profileDropdownList.appendChild(item);
        });

        profileStatus.textContent = `${profiles.length} profiles found`;
    });

    async function loadSelectedProfile() {
        if (!selectedProfilePath) {
            profileStatus.textContent = "No profile selected!";
            return;
        }

        profileStatus.textContent = "Loading autosave/info.sii...";
        const result = await invoke("load_profile", {
            profilePath: selectedProfilePath,
        });

        profileStatus.textContent = result;
        await updateAllDisplays();

        await invoke("read_save_config", {
            profilePath: selectedProfilePath,
        });
    }

    // Load all save data
    async function updateAllDisplays() {
        try {
            const data = await invoke("read_all_save_data");
            window.currentProfileData = data;

            if (moneyDisplay) moneyDisplay.textContent = `Geld: ${data.money.toLocaleString()} €`;
            if (xpDisplay) xpDisplay.textContent = `XP: ${data.xp.toLocaleString()}`;

            loadTools(activeTab);
        } catch (error) {
            console.error(error);
        }
    }

    // Save money
    if (moneyBtn) {
        moneyBtn.addEventListener("click", async () => {
            const amount = Number(document.querySelector("#money-input").value);
            editStatus.textContent = "Saving...";

            await invoke("edit_money", { amount });

            editStatus.textContent = "Money saved!";
            await updateAllDisplays();
        });
    }

    // Save XP
    if (levelBtn) {
        levelBtn.addEventListener("click", async () => {
            const xp = Number(document.querySelector("#level-input").value);
            editStatus.textContent = "Saving...";

            await invoke("edit_level", { xp });

            editStatus.textContent = "XP saved!";
            await updateAllDisplays();
        });
    }
});
