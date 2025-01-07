class Menu {
	menu_button: HTMLElement;
	menu_big: NodeListOf<ChildNode>;
	menu: HTMLElement;
	menu_open: HTMLElement;
	menu_close: HTMLElement;

	constructor() {
		this.menu_button = document.getElementById("menu_button");
		this.menu_big = document.getElementById("menu_big").childNodes;
		this.menu = document.getElementById("menu");
		this.menu_open = document.getElementById("menu_open");
		this.menu_close = document.getElementById("menu_close");

		this.menu_big.forEach((node) => {
			node.addEventListener("click", () => {
				this.menu_big.forEach((node) => {
					if (node instanceof HTMLElement) {
						node.classList.remove("menu_selected");
					}
				});
				if (node instanceof HTMLElement) {
					node.classList.add("menu_selected");
				}
			});
		});

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

window.addEventListener("DOMContentLoaded", () => {
	const menu: Menu = new Menu();
});
