const currentPath = window.location.pathname;
class Menu {
    menu_button;
    menu_big;
    menu;
    menu_open;
    menu_close;
    constructor(menu_labels) {
        this.menu_button = document.getElementById(menu_labels.button);
        this.menu_big = document.getElementById(menu_labels.big).childNodes;
        this.menu = document.getElementById(menu_labels.menu);
        this.menu_open = document.getElementById(menu_labels.open);
        this.menu_close = document.getElementById(menu_labels.close);
        // Menu item selection styling
        this.menu_big.forEach((node) => {
            node.addEventListener("click", () => {
                this.menu_big.forEach((node) => {
                    if (node instanceof HTMLElement) {
                        node.classList.remove(menu_labels.menu_selected);
                    }
                });
                if (node instanceof HTMLElement) {
                    node.classList.add(menu_labels.menu_selected);
                }
            });
        });
        // Mobile menu
        this.menu_button.addEventListener("click", () => {
            if (this.menu.classList.contains("hidden")) {
                this.menu.classList.add("flex");
                this.menu.classList.remove("hidden");
                this.menu_open.classList.add("hidden");
                this.menu_close.classList.remove("hidden");
            }
            else {
                this.menu.classList.add("hidden");
                this.menu.classList.remove("flex");
                this.menu_open.classList.remove("hidden");
                this.menu_close.classList.add("hidden");
            }
        });
        // Mobile when close when clicking outside the menu
        document.addEventListener("click", (e) => {
            if (e.target instanceof HTMLElement && !this.menu_button.contains(e.target) && this.menu.classList.contains("flex")) {
                this.menu.classList.add("hidden");
                this.menu.classList.remove("flex");
                this.menu_open.classList.remove("hidden");
                this.menu_close.classList.add("hidden");
            }
        });
    }
}
if (currentPath !== "/") {
    window.addEventListener("DOMContentLoaded", () => {
        const menu_labels = {
            button: "menu_button",
            big: "menu_big",
            menu: "menu",
            open: "menu_open",
            close: "menu_close",
            menu_selected: "menu_selected"
        };
        const menu = new Menu(menu_labels);
    });
}
// @ts-ignore
htmx.config.includeIndicatorStyles = false;
