const currentPath = window.location.pathname;

interface menuLabels {
	button: string;
	big: string,
	menu: string;
	open: string;
	close: string;
	menu_selected: string;
	mobile_menu_selected: string;
}

class Menu {
	menu_button: HTMLElement;
	menu_big: NodeListOf<ChildNode>;
	menu: HTMLElement;
	menu_mobile: NodeListOf<ChildNode>;
	menu_open: HTMLElement;
	menu_close: HTMLElement;

	constructor(menu_labels: menuLabels) {
		this.menu_button = document.getElementById(menu_labels.button);
		this.menu_big = document.getElementById(menu_labels.big).childNodes;
		this.menu = document.getElementById(menu_labels.menu);
		this.menu_mobile = document.getElementById(menu_labels.menu).childNodes;
		this.menu_open = document.getElementById(menu_labels.open);
		this.menu_close = document.getElementById(menu_labels.close);

		// Big menu item selection styling
		this.menu_big.forEach((node) => {
			node.addEventListener("click", () => {
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
			} else {
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
		const menu_labels: menuLabels = {
			button: "menu_button",
			big: "menu_big",
			menu: "menu",
			open: "menu_open",
			close: "menu_close",
			menu_selected: "menu_selected",
			mobile_menu_selected: "mobile_menu_selected"
		};

		new Menu(menu_labels);
	});
}

// @ts-ignore
htmx.config.includeIndicatorStyles = false;
