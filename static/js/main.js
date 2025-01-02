window.addEventListener("DOMContentLoaded", () => {
    const menu_button = document.getElementById("menu_button");
    const menu_big = document.getElementById("menu_big").childNodes;
    const menu = document.getElementById("menu");
    menu_big.forEach((node) => {
        node.addEventListener("click", (e) => {
            menu_big.forEach((node) => {
                if (node instanceof HTMLElement) {
                    node.classList.remove("menu_selected");
                }
            });
            if (node instanceof HTMLElement) {
                node.classList.add("menu_selected");
            }
        });
    });
    menu_button.addEventListener("click", () => {
        if (menu.classList.contains("hidden")) {
            menu.classList.add("flex");
            menu.classList.remove("hidden");
        }
        else {
            menu.classList.add("hidden");
            menu.classList.remove("flex");
        }
    });
    document.addEventListener("click", (e) => {
        if (e.target instanceof HTMLElement && !menu_button.contains(e.target) && menu.classList.contains("flex")) {
            menu.classList.add("hidden");
            menu.classList.remove("flex");
        }
    });
});
