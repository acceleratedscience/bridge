var currentPath = window.location.pathname;
var Menu = /** @class */ (function () {
    function Menu(menu_labels, main) {
        var _this = this;
        this.menu_button = document.getElementById(menu_labels.button);
        this.menu_big = document.getElementById(menu_labels.big).childNodes;
        this.menu = document.getElementById(menu_labels.menu);
        this.menu_mobile = document.getElementById(menu_labels.menu).childNodes;
        this.menu_open = document.getElementById(menu_labels.open);
        this.menu_close = document.getElementById(menu_labels.close);
        this.main = document.getElementById(main).childNodes;
        var showById = function (node, delimiter) {
            if (node instanceof HTMLElement) {
                var element = node.id;
                var id = element.split(delimiter)[1];
                var target = document.getElementById(id);
                if (target instanceof HTMLElement) {
                    target.classList.remove("hidden");
                }
            }
        };
        var hideAllMain = function () {
            _this.main.forEach(function (node) {
                if (node instanceof HTMLElement) {
                    node.classList.add("hidden");
                }
            });
        };
        // Big menu item selection styling
        this.menu_big.forEach(function (node) {
            node.addEventListener("click", function () {
                hideAllMain();
                showById(node, "_");
                _this.menu_big.forEach(function (node) {
                    if (node instanceof HTMLElement) {
                        node.classList.remove(menu_labels.menu_selected);
                    }
                });
                _this.menu_mobile.forEach(function (node) {
                    if (node instanceof HTMLElement) {
                        node.classList.remove(menu_labels.mobile_menu_selected);
                    }
                });
                if (node instanceof HTMLElement) {
                    node.classList.add(menu_labels.menu_selected);
                    var mobile_target = node.id.replace("big_", "mobile_");
                    document.getElementById(mobile_target).classList.add(menu_labels.mobile_menu_selected);
                }
            });
        });
        // mobile menu item selection styling
        this.menu_mobile.forEach(function (node) {
            node.addEventListener("click", function () {
                hideAllMain();
                showById(node, "_");
                _this.menu_mobile.forEach(function (node) {
                    if (node instanceof HTMLElement) {
                        node.classList.remove(menu_labels.mobile_menu_selected);
                    }
                });
                _this.menu_big.forEach(function (node) {
                    if (node instanceof HTMLElement) {
                        node.classList.remove(menu_labels.menu_selected);
                    }
                });
                if (node instanceof HTMLElement) {
                    node.classList.add(menu_labels.mobile_menu_selected);
                    var big_target = node.id.replace("mobile_", "big_");
                    document.getElementById(big_target).classList.add(menu_labels.menu_selected);
                }
            });
        });
        // Mobile menu
        this.menu_button.addEventListener("click", function () {
            if (_this.menu.classList.contains("hidden")) {
                _this.menu.classList.add("flex");
                _this.menu.classList.remove("hidden");
                _this.menu_open.classList.add("hidden");
                _this.menu_close.classList.remove("hidden");
            }
            else {
                _this.menu.classList.add("hidden");
                _this.menu.classList.remove("flex");
                _this.menu_open.classList.remove("hidden");
                _this.menu_close.classList.add("hidden");
            }
        });
        // Mobile when close when clicking outside the menu
        document.addEventListener("click", function (e) {
            if (e.target instanceof HTMLElement && !_this.menu_button.contains(e.target) && _this.menu.classList.contains("flex")) {
                _this.menu.classList.add("hidden");
                _this.menu.classList.remove("flex");
                _this.menu_open.classList.remove("hidden");
                _this.menu_close.classList.add("hidden");
            }
        });
    }
    return Menu;
}());
if (currentPath !== "/") {
    window.addEventListener("DOMContentLoaded", function () {
        var menu_labels = {
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
