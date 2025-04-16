const currentPath = window.location.pathname;
class Menu {
    menu_button;
    menu_big;
    menu;
    menu_mobile;
    menu_open;
    menu_close;
    main;
    constructor(menu_labels, main) {
        this.menu_button = document.getElementById(menu_labels.button);
        this.menu_big = document.getElementById(menu_labels.big).childNodes;
        this.menu = document.getElementById(menu_labels.menu);
        this.menu_mobile = document.getElementById(menu_labels.menu).childNodes;
        this.menu_open = document.getElementById(menu_labels.open);
        this.menu_close = document.getElementById(menu_labels.close);
        this.main = document.getElementById(main).childNodes;
        const showById = (node, delimiter) => {
            if (node instanceof HTMLElement) {
                const element = node.id;
                const id = element.split(delimiter)[1];
                const target = document.getElementById(id);
                if (target instanceof HTMLElement) {
                    target.classList.remove("hidden");
                }
            }
        };
        const hideAllMain = () => {
            this.main.forEach((node) => {
                if (node instanceof HTMLElement) {
                    node.classList.add("hidden");
                }
            });
        };
        // Big menu item selection styling
        this.menu_big.forEach((node) => {
            node.addEventListener("click", () => {
                hideAllMain();
                showById(node, "_");
                this.menu_big.forEach((node) => {
                    if (node instanceof HTMLElement) {
                        node.classList.remove(menu_labels.menu_selected);
                    }
                });
                this.menu_mobile.forEach((node) => {
                    if (node instanceof HTMLElement) {
                        node.classList.remove(menu_labels.mobile_menu_selected);
                    }
                });
                if (node instanceof HTMLElement) {
                    node.classList.add(menu_labels.menu_selected);
                    let mobile_target = node.id.replace("big_", "mobile_");
                    document.getElementById(mobile_target).classList.add(menu_labels.mobile_menu_selected);
                }
            });
        });
        // mobile menu item selection styling
        this.menu_mobile.forEach((node) => {
            node.addEventListener("click", () => {
                hideAllMain();
                showById(node, "_");
                this.menu_mobile.forEach((node) => {
                    if (node instanceof HTMLElement) {
                        node.classList.remove(menu_labels.mobile_menu_selected);
                    }
                });
                this.menu_big.forEach((node) => {
                    if (node instanceof HTMLElement) {
                        node.classList.remove(menu_labels.menu_selected);
                    }
                });
                if (node instanceof HTMLElement) {
                    node.classList.add(menu_labels.mobile_menu_selected);
                    let big_target = node.id.replace("mobile_", "big_");
                    document.getElementById(big_target).classList.add(menu_labels.menu_selected);
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
            menu_selected: "menu_selected",
            mobile_menu_selected: "mobile_menu_selected"
        };
        new Menu(menu_labels, "main");
    });
}
// @ts-ignore
htmx.config.includeIndicatorStyles = false;
