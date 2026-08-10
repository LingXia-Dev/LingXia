/**
 * Replaces a native <select> with a listbox this page can actually style.
 *
 * A platform select renders from the OS, so it ignores the page entirely: on a
 * dark surface it arrives light, at the wrong size, with the wrong corner
 * radius. Everything else here is drawn by the page, and one control that is
 * not shows up as the seam.
 *
 * The native element stays in the DOM as the source of truth. Reading and
 * writing `.value` keeps working, and every change dispatches `input` and
 * `change`, so nothing else in this app has to know a picker is involved.
 */
(function () {
  "use strict";

  var open = null;

  function closeOpen() {
    if (!open) return;
    open.root.classList.remove("open");
    open.button.setAttribute("aria-expanded", "false");
    open.list.hidden = true;
    open = null;
  }

  function labelFor(select) {
    var option = select.options[select.selectedIndex];
    // A select whose value is "" is a prompt, not a choice: the placeholder
    // from the markup reads better than an empty row.
    if (!option || option.value === "") {
      return select.dataset.picker || (option ? option.textContent : "");
    }
    return option.textContent;
  }

  function build(select) {
    var root = document.createElement("div");
    root.className = "picker";

    var button = document.createElement("button");
    button.type = "button";
    button.className = "picker-button";
    button.setAttribute("aria-haspopup", "listbox");
    button.setAttribute("aria-expanded", "false");
    button.setAttribute(
      "aria-label",
      select.getAttribute("aria-label") || select.dataset.picker || labelFor(select)
    );

    var text = document.createElement("span");
    text.className = "picker-value";
    button.appendChild(text);

    var caret = document.createElement("span");
    caret.className = "picker-caret";
    caret.setAttribute("aria-hidden", "true");
    button.appendChild(caret);

    var list = document.createElement("div");
    list.className = "picker-list";
    list.setAttribute("role", "listbox");
    if (select.id) {
      list.id = select.id + "-listbox";
      button.setAttribute("aria-controls", list.id);
    }
    list.hidden = true;

    root.appendChild(button);
    root.appendChild(list);
    select.parentNode.insertBefore(root, select);
    root.appendChild(select);

    function sync() {
      text.textContent = labelFor(select);
      text.classList.toggle("placeholder", select.value === "");
    }

    function render() {
      list.textContent = "";
      Array.prototype.forEach.call(select.options, function (option, index) {
        if (option.value === "" && select.options.length > 1) return;
        var item = document.createElement("button");
        item.type = "button";
        item.className = "picker-option";
        item.setAttribute("role", "option");
        item.textContent = option.textContent;
        item.dataset.index = String(index);
        item.setAttribute("aria-selected", index === select.selectedIndex ? "true" : "false");
        if (index === select.selectedIndex) {
          item.classList.add("selected");
        }
        item.addEventListener("click", function () {
          choose(index);
        });
        list.appendChild(item);
      });
    }

    function choose(index) {
      select.selectedIndex = index;
      sync();
      // Both, because listeners in this app are split between them and a
      // picker must be indistinguishable from the control it replaced.
      select.dispatchEvent(new Event("input", { bubbles: true }));
      select.dispatchEvent(new Event("change", { bubbles: true }));
      closeOpen();
      button.focus();
    }

    function show() {
      closeOpen();
      render();
      list.hidden = false;
      root.classList.add("open");
      button.setAttribute("aria-expanded", "true");
      open = { root: root, button: button, list: list };
      var selected = list.querySelector(".selected") || list.firstElementChild;
      if (selected) selected.focus();
    }

    button.addEventListener("click", function () {
      if (open && open.root === root) closeOpen();
      else show();
    });

    button.addEventListener("keydown", function (event) {
      if (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        show();
      }
    });

    list.addEventListener("keydown", function (event) {
      var items = Array.prototype.slice.call(list.children);
      var at = items.indexOf(document.activeElement);
      if (event.key === "Escape") {
        event.preventDefault();
        closeOpen();
        button.focus();
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        (items[at + 1] || items[0]).focus();
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        (items[at - 1] || items[items.length - 1]).focus();
      } else if (event.key === "Home") {
        event.preventDefault();
        items[0].focus();
      } else if (event.key === "End") {
        event.preventDefault();
        items[items.length - 1].focus();
      } else if (event.key === "Tab") {
        closeOpen();
      }
    });

    // The option list is rebuilt elsewhere (installed fonts arrive after a
    // native call), so the label has to follow whatever the app writes.
    select.addEventListener("change", sync);
    var observer = new MutationObserver(sync);
    observer.observe(select, { childList: true });

    sync();
  }

  /**
   * Ties a tick slider to the number field it shares a value with. The number
   * field keeps its id and stays the one the rest of the app reads, so this is
   * a second way to move the same setting rather than a second setting.
   */
  function pairSliders() {
    document.querySelectorAll('input[type="range"][data-for]').forEach(function (range) {
      var field = document.getElementById(range.dataset.for);
      if (!field) return;
      var steps = (Number(range.max) - Number(range.min)) / Number(range.step || 1);
      // One tick per labelled stop, so the track reads as a scale.
      var ticks = Math.max(1, Math.round(steps / 8));
      range.style.setProperty("--tick", 100 / Math.max(1, steps / ticks) + "%");

      range.addEventListener("input", function () {
        if (field.value === range.value) return;
        field.value = range.value;
        field.dispatchEvent(new Event("input", { bubbles: true }));
      });
      field.addEventListener("input", function () {
        if (range.value !== field.value) range.value = field.value;
      });
      // The field is filled from the host after load; follow it then too.
      new MutationObserver(function () { range.value = field.value; })
        .observe(field, { attributes: true, attributeFilter: ["value"] });
      var poll = setInterval(function () {
        if (field.value && range.value !== field.value) { range.value = field.value; clearInterval(poll); }
      }, 200);
      setTimeout(function () { clearInterval(poll); }, 6000);
    });
  }

  function attach() {
    document.querySelectorAll("select[data-picker]").forEach(build);
    pairSliders();
    document.addEventListener("pointerdown", function (event) {
      if (open && !open.root.contains(event.target)) closeOpen();
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", attach);
  } else {
    attach();
  }
})();
