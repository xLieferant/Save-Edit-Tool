const container = document.querySelector("#tool-container");
const navButtons = document.querySelectorAll(".nav-btn");

function loadTools(tab) {
    container.innerHTML = "";

    tools[tab].forEach(t => {
        const card = document.createElement("div");
        card.classList.add("tool-card");

        card.innerHTML = `
            <img src="${t.img}">
            <div class="tool-content">
                <h3>${t.title}</h3>
                <p>${t.desc}</p>
                <button>Open</button>
            </div>
        `;

        card.querySelector("button").addEventListener("click", t.action);

        container.appendChild(card);
    });
}

// default
loadTools("trailer");

// modal handling
const modal = document.querySelector("#modal");
const modalTitle = document.querySelector("#modalTitle");
const modalInput = document.querySelector("#modalInput");

function openModal(title, placeholder) {
    modalTitle.textContent = title;
    modalInput.placeholder = placeholder;
    modal.style.display = "flex";
}

document.querySelector("#modalCancel").onclick = () => modal.style.display = "none";
document.querySelector("#modalApply").onclick = () => modal.style.display = "none";

// tab switching
navButtons.forEach(btn => {
    btn.onclick = () => loadTools(btn.dataset.tab);
});
