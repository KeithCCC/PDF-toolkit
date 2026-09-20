import "./style.css";
import { outputFields, bindOutputFields, readOutputFields, outputSummary, type PdfSettings, type ExportResult } from "./output";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save, confirm } from "@tauri-apps/plugin-dialog";
type Comment = {
  id: string;
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
  font_size: number;
  color: string;
  background: string;
};
type Page = {
  id: string;
  source: string;
  index: number;
  width: number;
  height: number;
  rotation: number;
  comments: Comment[];
};
type Group = { id: string; name: string; pages: Page[]; pdf_settings: PdfSettings };
type Snapshot = {
  project: { groups: Group[] };
  names: Record<string, string>;
  canUndo: boolean;
  canRedo: boolean;
  dirty: boolean;
  recoveryError?: string;
};
let state: Snapshot = {
  project: { groups: [] },
  names: {},
  canUndo: false,
  canRedo: false,
  dirty: false,
};
let selected = new Set<string>(),
  anchor = "",
  activeGroup = "",
  size = 160,
  version = 0,
  busy = false;
let office = { word: false, excel: false };
const cache = new Map<string, string>();
let queue: Promise<unknown> = Promise.resolve();
const call = <T = any>(request: object): Promise<T> => {
  const work = queue.then(() => invoke<T>("dispatch", { request }));
  queue = work.catch(() => {});
  return work;
};
const esc = (s: unknown) =>
  String(s ?? "").replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ]!,
  );
const $ = <T extends Element = HTMLElement>(s: string) =>
  document.querySelector<T>(s)!;
const allPages = () => state.project.groups.flatMap((g) => g.pages);
const pageById = (id: string) => allPages().find((p) => p.id === id)!;
const icon = (name: string) =>
  `<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${({ file: '<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v5h5M9 12h6M9 16h6"/>', plus: '<path d="M12 5v14M5 12h14"/>', save: '<path d="M4 3h13l3 3v15H4zM8 3v6h8V3M8 21v-8h8v8"/>', undo: '<path d="M8 5 3 10l5 5M3 10h11a6 6 0 0 1 6 6"/>', redo: '<path d="m16 5 5 5-5 5M21 10H10a6 6 0 0 0-6 6"/>', left: '<path d="m7 3-4 4 4 4M3 7h10a8 8 0 1 1-7 12"/>', right: '<path d="m17 3 4 4-4 4M21 7H11a8 8 0 1 0 7 12"/>', trash: '<path d="M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7"/>', export: '<path d="M12 16V3m-4 4 4-4 4 4M5 13v8h14v-8"/>', copy: '<rect x="8" y="8" width="12" height="13" rx="2"/><path d="M16 8V3H3v13h5"/>', split: '<path d="M12 3v6M12 9l-7 7m7-7 7 7M5 12v4h4m6 0h4v-4"/>', comment: '<path d="M3 4h18v13H8l-5 4zM7 8h10M7 12h7"/>' } as Record<string, string>)[name] ?? ""}</svg>`;
function btn(action: string, label: string, ico?: string, disabled = false) {
  return `<button data-action="${action}" ${disabled ? "disabled" : ""}>${ico ? icon(ico) : ""}<span>${label}</span></button>`;
}
function toast(message: string, error = false) {
  $("#notice").textContent = message;
  $("#notice").classList.toggle("error", error);
  const d = $<HTMLDialogElement>("#dialog");
  if (d?.open && error) {
    let n = d.querySelector<HTMLElement>(".dialog-error");
    if (!n) {
      n = document.createElement("p");
      n.className = "dialog-error";
      n.setAttribute("role", "alert");
      d.prepend(n);
    }
    n.textContent = message;
  }
}
function shell() {
  document.querySelector("#app")!.innerHTML =
    `<div class="app-shell" data-theme="${localStorage.getItem("theme") ?? "light"}"><header><div class="menubar"><div class="brand">${icon("file")}<strong>PDF Toolkit</strong><span>Basic</span></div><div class="menus">${btn("open-project", "作業を開く")}${btn("save-project", "作業を保存", "save")}${btn("help", "使い方")}</div><select id="theme" aria-label="テーマ"><option value="light">ライト</option><option value="dark">ダーク</option><option value="system">システム</option></select></div><div class="primary-toolbar">${btn("import", "PDFを追加", "plus")}<div class="separator"></div>${btn("office", "Word / Excelから変換", "file")}<div class="spacer"></div>${btn("export", "書き出す", "export")}</div><div class="edit-toolbar" id="edit-toolbar"></div></header><div id="notice" role="status">PDFを追加して、ページを自由に整理できます。</div><div class="workspace"><aside id="sidebar"></aside><main id="main"></main></div><footer><span id="status"></span><span>Ctrl：複数選択・コピー　 Shift：範囲選択</span><label>表示サイズ <input id="size" type="range" min="100" max="250" value="160"></label></footer></div><dialog id="dialog"></dialog><div id="busy" hidden><div><div class="spinner"></div><strong id="busy-message">処理中…</strong><progress id="progress" max="100"></progress>${btn("cancel", "キャンセル")}</div></div>`;
  $("#theme").addEventListener("change", (e) => {
    let t = (e.target as HTMLSelectElement).value;
    $(".app-shell").setAttribute("data-theme", t);
    localStorage.setItem("theme", t);
  });
  $<HTMLSelectElement>("#theme").value =
    localStorage.getItem("theme") ?? "light";
  $("#size").addEventListener("input", (e) => {
    size = Number((e.target as HTMLInputElement).value);
    render();
  });
  document.addEventListener("click", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-action]");
    if (b && !b.hasAttribute("disabled"))
      void action(b.dataset.action!, b).catch((err) =>
        toast(String(err), true),
      );
  });
  document.addEventListener("keydown", (e) => {
    if (
      (e.target as Element).matches("input,textarea,select") ||
      $<HTMLDialogElement>("#dialog").open
    )
      return;
    if (e.ctrlKey && e.key === "a") {
      e.preventDefault();
      selected = new Set(allPages().map((p) => p.id));
      selection();
    }
    if (e.ctrlKey && e.key === "z") {
      e.preventDefault();
      void change({ op: e.shiftKey ? "redo" : "undo" });
    }
    if (e.key === "Delete") void change({ op: "delete", ids: [...selected] });
  });
}
function selection() {
  document.querySelectorAll<HTMLElement>(".page-card").forEach((el) => {
    el.classList.toggle("selected", selected.has(el.dataset.id!));
    el.setAttribute("aria-selected", String(selected.has(el.dataset.id!)));
  });
  $("#edit-toolbar").innerHTML =
    `${btn("undo", "元に戻す", "undo", !state.canUndo)}${btn("redo", "やり直す", "redo", !state.canRedo)}<div class="separator"></div>${btn("rotate-left", "左へ回転", "left", !selected.size)}${btn("rotate-right", "右へ回転", "right", !selected.size)}${btn("transfer", "移動 / コピー", "copy", !selected.size)}${btn("extract", "抽出・分割", "split", !selected.size)}${btn("delete", "削除", "trash", !selected.size)}<div class="spacer"></div>${btn("range", "範囲選択")}`;
  $("#status").textContent =
    `${state.project.groups.length} グループ · ${allPages().length} ページ · ${selected.size} 選択${state.dirty ? " · 作業に変更あり" : ""}`;
}
function paper(p: Page, width: number) {
  const rot = p.rotation % 180 !== 0;
  const scale = width / (rot ? p.height : p.width);
  const w = p.width * scale,
    h = p.height * scale;
  const ow = rot ? h : w,
    oh = rot ? w : h;
  return `<div class="paper-wrap" style="width:${ow}px;height:${oh}px"><div class="paper" style="width:${w}px;height:${h}px;left:${(ow - w) / 2}px;top:${(oh - h) / 2}px;transform:rotate(${p.rotation}deg)"><img class="page-image" data-page="${p.id}" alt="ページ ${p.index + 1}" draggable="false">${p.comments.map((c) => `<img class="annotation" data-comment="${esc(c.id)}" data-cpage="${p.id}" style="left:${c.x * scale}px;top:${c.y * scale}px;width:${c.width * scale}px;height:${c.height * scale}px" alt="${esc(c.text)}" draggable="false">`).join("")}</div></div>`;
}
let observer: IntersectionObserver | undefined;
let dragFinishedAt = 0;
// Native file drops stay enabled in Tauri. Internal drags use pointer events,
// avoiding competition between Windows OLE file drops and HTML drag-and-drop.
function bindInternalDragging() {
  document
    .querySelectorAll<HTMLElement>(".page-card,.group-header")
    .forEach((el) => {
      el.draggable = false;
      el.addEventListener("pointerdown", (start) => {
        if (
          start.button !== 0 ||
          busy ||
          (start.target as Element).closest("input,button")
        )
          return;
        el.setPointerCapture(start.pointerId);
        let moved = false,
          destination: HTMLElement | null = null;
        const move = (event: PointerEvent) => {
          if (
            !moved &&
            Math.hypot(
              event.clientX - start.clientX,
              event.clientY - start.clientY,
            ) < 6
          )
            return;
          if (!moved) {
            moved = true;
            if (el.dataset.id && !selected.has(el.dataset.id)) {
              selected = new Set([el.dataset.id]);
              selection();
            }
          }
          destination?.classList.remove("drop-before");
          destination =
            document
              .elementFromPoint(event.clientX, event.clientY)
              ?.closest<HTMLElement>(".page-card,.end-drop,.group-header") ??
            null;
          destination?.classList.add("drop-before");
          const bounds = $("#main").getBoundingClientRect();
          if (event.clientY > bounds.bottom - 40) $("#main").scrollTop += 16;
          if (event.clientY < bounds.top + 40) $("#main").scrollTop -= 16;
        };
        const finish = (event: PointerEvent) => {
          el.removeEventListener("pointermove", move);
          el.removeEventListener("pointerup", finish);
          el.removeEventListener("pointercancel", finish);
          destination?.classList.remove("drop-before");
          if (!moved) return;
          dragFinishedAt = Date.now();
          if (event.type === "pointercancel" || !destination) return;
          const target =
            destination.dataset.group ??
            destination.dataset.target ??
            destination.dataset.dragGroup!;
          const group = state.project.groups.find((g) => g.id === target);
          if (!group) return;
          if (el.dataset.dragGroup) {
            if (el.dataset.dragGroup !== target)
              void change({
                op: "merge",
                source: el.dataset.dragGroup,
                target,
              });
          } else
            void change({
              op: "transfer",
              ids: [...selected],
              target,
              at: destination.dataset.index
                ? Number(destination.dataset.index)
                : group.pages.length,
              copy: event.ctrlKey,
            });
        };
        el.addEventListener("pointermove", move);
        el.addEventListener("pointerup", finish);
        el.addEventListener("pointercancel", finish);
      });
    });
}
function render() {
  version++;
  const current = version;
  observer?.disconnect();
  selected = new Set([...selected].filter((id) => pageById(id)));
  if (!state.project.groups.some((g) => g.id === activeGroup))
    activeGroup = state.project.groups[0]?.id ?? "";
  $("#sidebar").innerHTML =
    `<div class="side-title">出力グループ <span>${state.project.groups.length}</span></div>${state.project.groups.map((g, i) => `<button class="group-nav ${g.id === activeGroup ? "active" : ""}" data-action="group" data-id="${g.id}">${icon("file")}<span>${esc(g.name)}<small>${g.pages.length} ページ · PDF ${i + 1}</small></span></button>`).join("")}<div class="side-hint">1グループが1つのPDFになります。<br>ページをドラッグして整理できます。</div>`;
  $("#main").innerHTML =
    !allPages().length && !state.project.groups.length
      ? `<div class="empty"><div class="empty-icon">${icon("file")}</div><h1>PDFを、思いどおりの順番に。</h1><p>ファイルをここにドロップするか、<br>PDFを追加してページの整理を始めましょう。</p>${btn("import", "PDFを追加", "plus")}<button class="text-button" data-action="sample">サンプルで試す</button><div class="local-note">ファイルはこのPCで処理されます</div></div>`
      : state.project.groups
          .map(
            (g, gi) =>
              `<section class="group" id="group-${g.id}" data-group="${g.id}"><div class="group-header" draggable="true" data-drag-group="${g.id}"><span class="group-number">${String(gi + 1).padStart(2, "0")}</span><input class="group-name" data-group-name="${g.id}" value="${esc(g.name)}" aria-label="出力ファイル名"><span class="extension">.pdf</span><span class="badge">${g.pages.length} ページ</span><div class="spacer"></div><button data-action="group-export" data-id="${g.id}" title="このグループを書き出す">${icon("export")}</button><button data-action="remove-group" data-id="${g.id}" title="グループを削除">${icon("trash")}</button></div><div class="pages" data-target="${g.id}">${g.pages.map((p, i) => `<div class="page-unit"><button class="split-point" data-action="split" data-group="${g.id}" data-at="${i}" title="ここで分割" ${i === 0 ? "disabled" : ""}>┊</button><article class="page-card" role="option" tabindex="0" aria-label="${esc(g.name)} ページ ${i + 1}" data-id="${p.id}" data-group="${g.id}" data-index="${i}" draggable="true">${paper(p, size)}<div class="page-caption"><strong>${i + 1}</strong><span>${esc(state.names[p.source] ?? "元PDF")}<small>元ページ ${p.index + 1}${p.comments.length ? " · コメント " + p.comments.length : ""}</small></span></div></article></div>`).join("")}<div class="end-drop" data-target="${g.id}">${g.pages.length ? "ここへ移動" : "ページをここへドロップ"}</div></div></section>`,
          )
          .join("");
  selection();
  document
    .querySelectorAll<HTMLInputElement>("[data-group-name]")
    .forEach((el) =>
      el.addEventListener(
        "change",
        () =>
          void change({
            op: "rename",
            id: el.dataset.groupName,
            name: el.value,
          }),
      ),
    );
  document.querySelectorAll<HTMLElement>(".page-card").forEach((el) => {
    el.addEventListener("click", (e) => {
      if (Date.now() - dragFinishedAt < 300) return;
      activeGroup = el.dataset.group!;
      const id = el.dataset.id!;
      if (e.shiftKey && anchor) {
        const ids = allPages().map((p) => p.id),
          a = ids.indexOf(anchor),
          b = ids.indexOf(id);
        if (a >= 0)
          selected = new Set(ids.slice(Math.min(a, b), Math.max(a, b) + 1));
      } else if (e.ctrlKey) {
        if (selected.has(id)) selected.delete(id);
        else selected.add(id);
      } else selected = new Set([id]);
      anchor = id;
      selection();
    });
    el.addEventListener("dblclick", () => void preview(el.dataset.id!));
    el.addEventListener("keydown", (e) => {
      if (e.key === "Enter") void preview(el.dataset.id!);
    });
    el.addEventListener("dragstart", (e) => {
      if (!selected.has(el.dataset.id!)) {
        selected = new Set([el.dataset.id!]);
        selection();
      }
      e.dataTransfer?.setData(
        "application/pdf-toolkit",
        JSON.stringify({ ids: [...selected] }),
      );
      if (e.dataTransfer) e.dataTransfer.effectAllowed = "copyMove";
    });
    el.addEventListener("dragover", (e) => {
      e.preventDefault();
      el.classList.add("drop-before");
    });
    el.addEventListener("dragleave", () => el.classList.remove("drop-before"));
    el.addEventListener("drop", (e) => {
      e.preventDefault();
      e.stopPropagation();
      el.classList.remove("drop-before");
      const data = e.dataTransfer?.getData("application/pdf-toolkit");
      if (data) {
        const obj = JSON.parse(data);
        void change(
          obj.group
            ? { op: "merge", source: obj.group, target: el.dataset.group }
            : {
                op: "transfer",
                ids: obj.ids,
                target: el.dataset.group,
                at: Number(el.dataset.index),
                copy: e.ctrlKey,
              },
        );
      }
    });
  });
  document.querySelectorAll<HTMLElement>("[data-drag-group]").forEach((el) =>
    el.addEventListener("dragstart", (e) => {
      e.dataTransfer?.setData(
        "application/pdf-toolkit",
        JSON.stringify({ group: el.dataset.dragGroup }),
      );
    }),
  );
  document
    .querySelectorAll<HTMLElement>(".end-drop,.group-header")
    .forEach((el) => {
      el.addEventListener("dragover", (e) => e.preventDefault());
      el.addEventListener("drop", (e) => {
        e.preventDefault();
        const data = e.dataTransfer?.getData("application/pdf-toolkit");
        if (!data) return;
        const obj = JSON.parse(data),
          gid = el.dataset.target ?? el.dataset.dragGroup!,
          g = state.project.groups.find((g) => g.id === gid)!;
        void change(
          obj.group
            ? { op: "merge", source: obj.group, target: gid }
            : {
                op: "transfer",
                ids: obj.ids,
                target: gid,
                at: g.pages.length,
                copy: e.ctrlKey,
              },
        );
      });
    });
  bindInternalDragging();
  observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries)
        if (entry.isIntersecting) {
          observer?.unobserve(entry.target);
          void loadPaper(entry.target as HTMLElement, current);
        }
    },
    { root: $("#main"), rootMargin: "150px" },
  );
  document
    .querySelectorAll<HTMLElement>(".page-card")
    .forEach((el) => observer!.observe(el));
}
async function loadPaper(root: HTMLElement, generation?: number) {
  if (cache.size > 160) {
    for (const key of [...cache.keys()].slice(0, 80)) cache.delete(key);
  }
  for (const el of root.querySelectorAll<HTMLImageElement>(".page-image")) {
    const p = pageById(el.dataset.page!);
    if (!p) continue;
    const width = root.classList.contains("preview-paper") ? 1400 : 400,
      key = `${p.source}:${p.index}:${width}`;
    try {
      if (!cache.has(key))
        cache.set(
          key,
          await call<string>({ op: "thumbnail", id: p.id, width }),
        );
      if (generation !== undefined && generation !== version) return;
      el.src = cache.get(key)!;
    } catch (err) {
      toast(String(err), true);
    }
  }
  for (const el of root.querySelectorAll<HTMLImageElement>(".annotation")) {
    const c = pageById(el.dataset.cpage!)?.comments.find(
      (c) => c.id === el.dataset.comment,
    );
    if (!c) continue;
    const key = JSON.stringify(c);
    if (!cache.has(key))
      cache.set(key, await call<string>({ op: "comment_image", comment: c }));
    el.src = cache.get(key)!;
  }
}
async function work<T>(message: string, fn: () => Promise<T>): Promise<T> {
  busy = true;
  $("#busy").hidden = false;
  $("#busy-message").textContent = message;
  try {
    return await fn();
  } finally {
    busy = false;
    $("#busy").hidden = true;
  }
}
async function change(req: object) {
  try {
    state = await work("作業を更新しています…", () => call<Snapshot>(req));
    render();
    toast(
      state.recoveryError
        ? "復旧用保存に失敗しました。作業を手動で保存してください：" +
            state.recoveryError
        : "変更を反映し、復旧用データを自動保存しました。",
      !!state.recoveryError,
    );
    return true;
  } catch (err) {
    toast(String(err), true);
    return false;
  }
}
function closeDialog() {
  const d = $<HTMLDialogElement>("#dialog");
  if (d.open) d.close();
  d.innerHTML = "";
}
function dialog(title: string, body: string, wide = false) {
  closeDialog();
  const d = $<HTMLDialogElement>("#dialog");
  const theme = getComputedStyle($(".app-shell"));
  for (const key of [
    "bar-bg",
    "text",
    "muted",
    "strong-border",
    "border",
    "active-bg",
    "focus-ring",
  ])
    d.style.setProperty("--" + key, theme.getPropertyValue("--" + key));
  d.style.colorScheme = theme.colorScheme;
  d.className = wide ? "wide" : "";
  d.innerHTML = `<div class="dialog-title"><h2>${esc(title)}</h2><button data-action="close-dialog" aria-label="閉じる">×</button></div>${body}`;
  d.showModal();
  return d;
}
function askText(
  title: string,
  label: string,
  value = "",
  password = false,
): Promise<string | null> {
  return new Promise((resolve) => {
    const d = dialog(
      title,
      `<form id="text-form"><label>${esc(label)}<input id="text-value" type="${password ? "password" : "text"}" value="${esc(value)}" autofocus></label><div class="dialog-actions"><button type="button" id="text-cancel">キャンセル</button><button class="primary" type="submit">OK</button></div></form>`,
    );
    let done = false;
    const finish = (v: string | null) => {
      if (done) return;
      done = true;
      closeDialog();
      resolve(v);
    };
    $("#text-form").addEventListener("submit", (e) => {
      e.preventDefault();
      finish($<HTMLInputElement>("#text-value").value);
    });
    $("#text-cancel").onclick = () => finish(null);
    d.addEventListener("close", () => finish(null), { once: true });
  });
}
async function importPaths(paths: string[]) {
  for (const path of paths) {
    if (!path.toLowerCase().endsWith(".pdf")) {
      toast(
        "PDFファイルを選択してください。Officeは変換ボタンを使用します。",
        true,
      );
      continue;
    }
    try {
      state = await work("PDFを読み込んでいます…", () =>
        call({ op: "import", path }),
      );
    } catch (err) {
      const password = await askText(
        "PDFを開けませんでした",
        `${String(err)}\nパスワードが必要なPDFの場合は入力してください。`,
        "",
        true,
      );
      if (password !== null) {
        try {
          state = await work("PDFを読み込んでいます…", () =>
            call({ op: "import", path, password }),
          );
        } catch (e) {
          toast(String(e), true);
        }
      }
    }
    render();
  }
}
async function exportDialog(groups: string[] = []) {
  if (!allPages().length) return;
  await call({op:"discard_export"});
  const chosen = state.project.groups.filter(g=>!groups.length || groups.includes(g.id));
  const defaults: PdfSettings = {...(chosen[0]?.pdf_settings ?? {mode:"off",target_mb:"10",protect:false}), protect:chosen.some(g=>g.pdf_settings?.protect)};
  dialog(
    "書き出し",
    `<p>対象：${groups.length ? "選択したグループ" : "すべてのグループ"}。元ファイルは変更しません。</p><div class="export-groups">${state.project.groups
      .filter((g) => g.pages.length)
      .map(
        (g) =>
          `<label class="check"><input type="checkbox" name="export-group" value="${g.id}" ${!groups.length || groups.includes(g.id) ? "checked" : ""}>${esc(g.name)} (${g.pages.length}ページ)</label>`,
      )
      .join(
        "",
      )}</div><label>形式<select id="format"><option value="pdf">PDF</option><option value="docx">Word（ページ画像）</option><option value="xlsx">Excel（1ページ＝1シート）</option></select></label><label id="office-quality">画像の品質<select id="quality"><option value="standard">標準 · 150 dpi</option><option value="high">高画質 · 300 dpi</option></select></label>${outputFields(defaults)}<div class="dialog-actions"><button id="prepare-export" class="primary">変換結果を確認</button></div>`,
  );
  bindOutputFields();
  document.querySelectorAll<HTMLInputElement>('input[name="export-group"]').forEach(box=>box.addEventListener('change',()=>{
    if(box.checked && state.project.groups.find(g=>g.id===box.value)?.pdf_settings.protect) {
      const protect=$<HTMLInputElement>('#protect-pdf');
      protect.checked=true;
      protect.dispatchEvent(new Event('change'));
    }
  }));
  $("#prepare-export").onclick = async () => {
    const format = $<HTMLSelectElement>("#format").value,
      quality = $<HTMLSelectElement>("#quality").value;
    groups = [
      ...document.querySelectorAll<HTMLInputElement>(
        'input[name="export-group"]:checked',
      ),
    ].map((el) => el.value);
    if (!groups.length) {
      toast("出力するグループを選択してください。", true);
      return;
    }
    let password: string | undefined;
    if(format === 'pdf') {
      try {
        const input = readOutputFields();
        if(!await change({op:'set_output_settings',groups,settings:input.settings})) return;
        password = input.password;
      } catch(err) { toast(String(err),true); return; }
    }
    closeDialog();
    try {
      const result = await work("出力を準備しています…", () =>
        call<ExportResult>({
          op: "prepare_export",
          format,
          quality,
          groups,
          password,
        }),
      );
      password = undefined;
      const unmet = result.reports.some(r=>r.met_target===false);
      const confirmDialog = dialog(
        "出力内容の確認",
        `<p>${result.names.map(esc).join(" / ")}</p>${format==='pdf'?`<label>比較するPDF<select id="compare-group">${result.names.map((name,i)=>`<option value="${i}">${esc(name)}</option>`).join('')}</select></label><div id="output-summary"></div><div class="field-grid"><label>ページ<input id="compare-page" type="number" min="1" max="${result.pageCounts[0]}" value="1"></label><label>比較倍率<select id="compare-width"><option value="500">小</option><option value="900" selected>標準</option><option value="1400">拡大</option></select></label></div><div class="comparison"><figure><figcaption>圧縮前</figcaption><img id="compare-before" alt="圧縮前のページ"></figure><figure><figcaption>最終出力</figcaption><img id="compare-after" alt="最終出力の同じページ"></figure></div>`:`<div class="export-previews">${result.previews.map((src,i)=>`<figure><img src="${src}" alt="変換結果 ${i+1}"></figure>`).join('')}</div>`}${unmet?'<label class="check"><input id="acknowledge-unmet" type="checkbox">目標未達のPDFがあります。実サイズを了承して保存します。</label>':''}<div class="dialog-actions"><button id="retry-export">設定を変更</button><button id="finish-export" class="primary" ${unmet?'disabled':''}>保存先を選んで書き出す</button></div>`,
        true,
      );
      confirmDialog.addEventListener('close',()=>{void call({op:'discard_export'});},{once:true});
      $('#retry-export').onclick=()=>{void exportDialog(groups);};
      if(unmet) $('#acknowledge-unmet').onchange=()=>{$<HTMLButtonElement>('#finish-export').disabled=!$<HTMLInputElement>('#acknowledge-unmet').checked;};
      if(format==='pdf') {
        let generation=0;
        const updateComparison=async()=>{
          const revision=++generation;
          const group=Number($<HTMLSelectElement>('#compare-group').value);
          const page=$<HTMLInputElement>('#compare-page');
          page.max=String(result.pageCounts[group]);
          page.value=String(Math.min(result.pageCounts[group],Math.max(1,Math.trunc(Number(page.value)||1))));
          $('#output-summary').innerHTML=outputSummary(result.reports[group]);
          const width=Number($<HTMLSelectElement>('#compare-width').value);
          try {
            const images=await call<{before:string;after:string}>({op:'output_preview',token:result.token,group,index:Number(page.value)-1,width});
            if(revision!==generation || !confirmDialog.open || !document.getElementById('compare-before')) return;
            for(const side of ['before','after'] as const) {
              const img=$<HTMLImageElement>(`#compare-${side}`); img.src=images[side]; img.style.width=`${width/2}px`;
            }
          } catch(err) {if(confirmDialog.open) toast(String(err),true);}
        };
        for(const id of ['compare-group','compare-page','compare-width']) $(`#${id}`).addEventListener('change',()=>void updateComparison());
        await updateComparison();
      }
      $("#finish-export").onclick = async () => {
        try {
          const path = await open({
            directory: true,
            multiple: false,
            title: "保存先フォルダー",
          });
          if (!path) return;
          const conflicts = await call<string[]>({
            op: "export_conflicts",
            path,
          });
          if (
            conflicts.length &&
            !(await confirm(
              `同名ファイルを上書きしますか？\n${conflicts.join("\n")}`,
              { title: "上書きの確認", kind: "warning" },
            ))
          )
            return;
          await work("ファイルを書き出しています…", () =>
            call({
              op: "finish_export",
              path,
              overwrite: conflicts.length > 0,
              token: result.token,
              acknowledgeUnmet: !unmet || $<HTMLInputElement>('#acknowledge-unmet').checked,
            }),
          );
          closeDialog();
          toast(`保存しました：${path}`);
        } catch (err) {
          toast(String(err), true);
        }
      };
    } catch (err) {
      toast(String(err), true);
    } finally {
      password = undefined;
    }
  };
}
async function preview(id: string) {
  const p = pageById(id);
  if (!p) return;
  const width = Math.min(660, p.rotation % 180 ? p.height : p.width);
  dialog(
    `ページ ${p.index + 1} · プレビューとコメント`,
    `<div class="preview-layout"><div class="preview-paper">${paper(p, width)}</div><form id="comment-form"><h3>文字コメント</h3><select id="comment-select"><option value="">新しいコメント</option>${p.comments.map((c, i) => `<option value="${c.id}">${i + 1}. ${esc(c.text.slice(0, 24))}</option>`).join("")}</select><label>内容<textarea id="comment-text" rows="4" required></textarea></label><div class="field-grid">${[
      ["x", "左", 30],
      ["y", "上", 40],
      ["width", "幅", Math.min(180, p.width - 30)],
      ["height", "高さ", 70],
      ["font_size", "文字サイズ", 14],
    ]
      .map(
        ([k, label, value]) =>
          `<label>${label} (pt)<input id="c-${k}" type="number" min="${k === "font_size" ? 6 : 0}" value="${value}" step="1"></label>`,
      )
      .join(
        "",
      )}<label>文字色<input id="c-color" type="color" value="#202020"></label><label>背景色<input id="c-background" type="color" value="#fff6c4"></label></div><p class="hint">位置・サイズは回転前のページを基準に指定します。コメントをドラッグして移動することもできます。</p><div class="dialog-actions"><button id="delete-comment" type="button">削除</button><button type="submit" class="primary">反映</button></div></form></div>`,
    true,
  );
  await loadPaper($(".preview-paper"));
  const choose = (cid: string) => {
    const c = p.comments.find((c) => c.id === cid);
    $<HTMLSelectElement>("#comment-select").value = cid;
    if (c) {
      $<HTMLTextAreaElement>("#comment-text").value = c.text;
      for (const k of [
        "x",
        "y",
        "width",
        "height",
        "font_size",
        "color",
        "background",
      ] as const)
        $<HTMLInputElement>(`#c-${k}`).value = String(c[k]);
    }
  };
  $("#comment-select").addEventListener("change", () =>
    choose($<HTMLSelectElement>("#comment-select").value),
  );
  $("#comment-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const c: Comment = {
      id: $<HTMLSelectElement>("#comment-select").value,
      text: $<HTMLTextAreaElement>("#comment-text").value,
      x: 0,
      y: 0,
      width: 0,
      height: 0,
      font_size: 14,
      color: $<HTMLInputElement>("#c-color").value,
      background: $<HTMLInputElement>("#c-background").value,
    };
    for (const k of ["x", "y", "width", "height", "font_size"] as const)
      c[k] = Number($<HTMLInputElement>(`#c-${k}`).value);
    if (await change({ op: "comment", page: id, comment: c }))
      await preview(id);
  });
  $("#delete-comment").onclick = async () => {
    const cid = $<HTMLSelectElement>("#comment-select").value;
    if (cid) {
      await change({ op: "delete_comment", page: id, id: cid });
      await preview(id);
    }
  };
  document
    .querySelectorAll<HTMLElement>(".preview-paper .annotation")
    .forEach((el) => {
      el.style.cursor = "move";
      el.addEventListener("pointerdown", (ev) => {
        ev.preventDefault();
        const c = p.comments.find((c) => c.id === el.dataset.comment)!;
        choose(c.id);
        const startX = ev.clientX,
          startY = ev.clientY,
          scale = width / (p.rotation % 180 ? p.height : p.width);
        el.setPointerCapture(ev.pointerId);
        let nx = c.x,
          ny = c.y;
        const move = (e: PointerEvent) => {
          const dx = (e.clientX - startX) / scale,
            dy = (e.clientY - startY) / scale;
          const angle = (-p.rotation * Math.PI) / 180;
          nx = Math.min(
            p.width - c.width,
            Math.max(0, c.x + dx * Math.cos(angle) - dy * Math.sin(angle)),
          );
          ny = Math.min(
            p.height - c.height,
            Math.max(0, c.y + dx * Math.sin(angle) + dy * Math.cos(angle)),
          );
          el.style.left = `${nx * scale}px`;
          el.style.top = `${ny * scale}px`;
        };
        el.onpointermove = move;
        el.onpointerup = async () => {
          el.onpointermove = null;
          el.onpointerup = null;
          if (
            await change({
              op: "comment",
              page: id,
              comment: { ...c, x: nx, y: ny },
            })
          )
            await preview(id);
        };
      });
    });
}
async function officeImport() {
  const extensions = [
    ...(office.word ? ["docx"] : []),
    ...(office.excel ? ["xlsx"] : []),
  ];
  if (!extensions.length) {
    toast("デスクトップ版WordまたはExcelが必要です。", true);
    return;
  }
  const path = await open({
    multiple: false,
    filters: [{ name: "Office", extensions }],
  });
  if (!path) return;
  let sheets: string[] = [];
  if (path.toLowerCase().endsWith(".xlsx")) {
    const list = await work("シート一覧を取得しています…", () =>
      call<{ name: string; visible: boolean }[]>({ op: "sheets", path }),
    );
    const selectedSheets = await new Promise<string[] | null>((resolve) => {
      const d = dialog(
        "出力するシート",
        `<form id="sheets-form">${list.map((s, i) => `<label class="check"><input type="checkbox" name="sheet" value="${i}" ${s.visible ? "checked" : "disabled"}>${esc(s.name)}${s.visible ? "" : "（非表示）"}</label>`).join("")}<div class="dialog-actions"><button type="submit" class="primary">変換する</button></div></form>`,
      );
      let done = false;
      $("#sheets-form").onsubmit = (e) => {
        e.preventDefault();
        const names = [
          ...d.querySelectorAll<HTMLInputElement>("input:checked"),
        ].map((e) => list[Number(e.value)].name);
        if (!names.length) return;
        done = true;
        closeDialog();
        resolve(names);
      };
      d.addEventListener(
        "close",
        () => {
          if (!done) resolve(null);
        },
        { once: true },
      );
    });
    if (!selectedSheets) return;
    sheets = selectedSheets;
  }
  const result = await work("OfficeでPDFに変換しています…", () =>
    call<{ preview: string; pages: number }>({
      op: "prepare_office",
      path,
      sheets,
    }),
  );
  dialog(
    "変換したPDFの確認",
    `<p>${result.pages} ページ</p><div class="office-preview"><img id="office-preview-image" src="${result.preview}" alt="Office変換結果"></div><label>ページ<input id="office-page" type="number" min="1" max="${result.pages}" value="1"></label><div class="dialog-actions"><button id="accept-office" class="primary">作業に追加</button></div>`,
    true,
  );
  $("#office-page").addEventListener("change", async (e) => {
    $<HTMLImageElement>("#office-preview-image").src = await call({
      op: "office_preview",
      index: Number((e.target as HTMLInputElement).value) - 1,
    });
  });
  $("#accept-office").onclick = async () => {
    closeDialog();
    await change({ op: "accept_office" });
  };
}
async function action(name: string, el: HTMLElement) {
  if (name === "cancel") {
    await invoke("cancel");
    $("#busy-message").textContent =
      "キャンセルしています。Office処理中は終了までお待ちください…";
    return;
  }
  if (busy) return;
  switch (name) {
    case "import": {
      const paths = await open({
        multiple: true,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (paths) await importPaths(paths);
      break;
    }
    case "sample":
      await change({ op: "sample" });
      break;
    case "undo":
    case "redo":
      await change({ op: name });
      break;
    case "rotate-left":
    case "rotate-right":
      await change({
        op: "rotate",
        ids: [...selected],
        delta: name === "rotate-left" ? -90 : 90,
      });
      break;
    case "delete":
      await change({ op: "delete", ids: [...selected] });
      break;
    case "group":
      activeGroup = el.dataset.id!;
      $(`#group-${activeGroup}`).scrollIntoView({ behavior: "smooth" });
      break;
    case "group-export":
      await exportDialog([el.dataset.id!]);
      break;
    case "export":
      await exportDialog();
      break;
    case "split":
      await change({
        op: "split",
        group: el.dataset.group,
        at: Number(el.dataset.at),
      });
      break;
    case "remove-group":
      if (
        await confirm(
          "このグループを作業から削除しますか？ 元に戻す操作で復旧できます。",
          { title: "グループの削除" },
        )
      )
        await change({ op: "remove_group", id: el.dataset.id });
      break;
    case "close-dialog":
      closeDialog();
      break;
    case "extract":
      dialog(
        "選択ページからグループを作成",
        `<p>${selected.size} ページを新しいグループにします。</p><div class="dialog-actions"><button id="extract-copy">コピーして抽出</button><button id="extract-move" class="primary">移動して分割</button></div>`,
      );
      for (const copy of [false, true])
        $(`#extract-${copy ? "copy" : "move"}`).onclick = async () => {
          closeDialog();
          await change({ op: "extract", ids: [...selected], copy });
        };
      break;
    case "transfer":
      dialog(
        "ページの移動・コピー",
        `<label>移動先<select id="transfer-target">${state.project.groups.map((g) => `<option value="${g.id}">${esc(g.name)}</option>`).join("")}</select></label><div class="dialog-actions"><button id="transfer-copy">末尾にコピー</button><button id="transfer-move" class="primary">末尾に移動</button></div>`,
      );
      for (const copy of [false, true])
        $(`#transfer-${copy ? "copy" : "move"}`).onclick = async () => {
          const target = $<HTMLSelectElement>("#transfer-target").value,
            at = state.project.groups.find((g) => g.id === target)!.pages
              .length;
          closeDialog();
          await change({
            op: "transfer",
            ids: [...selected],
            target,
            at,
            copy,
          });
        };
      break;
    case "range": {
      const g = state.project.groups.find((g) => g.id === activeGroup);
      if (!g) return;
      const value = await askText("ページ範囲を選択", `${g.name}：例 1,3,5-7`);
      if (value) {
        const ids = new Set<string>();
        for (const segment of value.split(",")) {
          const match = segment.trim().match(/^(\d+)(?:-(\d+))?$/);
          if (!match) throw Error("範囲の書式が不正です");
          const a = Number(match[1]),
            b = Number(match[2] ?? a);
          if (a < 1 || b < a || b > g.pages.length)
            throw Error("ページ範囲が不正です");
          for (let n = a; n <= b; n++) ids.add(g.pages[n - 1].id);
        }
        selected = ids;
        selection();
      }
      break;
    }
    case "save-project": {
      const path = await save({
        defaultPath: "作業.pdftk",
        filters: [{ name: "PDF Toolkit 作業", extensions: ["pdftk"] }],
      });
      if (path) {
        state = await work("作業を保存しています…", () =>
          call({ op: "save_project", path }),
        );
        selection();
        toast("作業を保存しました。");
      }
      break;
    }
    case "open-project": {
      if (
        state.dirty &&
        !(await confirm("現在の変更を閉じて、別の作業を開きますか？", {
          title: "作業を開く",
        }))
      )
        return;
      const path = await open({
        multiple: false,
        filters: [{ name: "PDF Toolkit 作業", extensions: ["pdftk"] }],
      });
      if (path) {
        cache.clear();
        await change({ op: "open_project", path });
      }
      break;
    }
    case "office":
      await officeImport();
      break;
    case "help":
      dialog(
        "PDF Toolkit の使い方",
        `<div class="help"><p>1. PDFを追加すると、ファイルごとに出力グループができます。</p><p>2. ページをドラッグして並べ替えます。Ctrlでコピー、Shiftで範囲選択できます。</p><p>3. ページ間の区切りを押すと分割できます。グループ見出しを別グループへドラッグすると結合できます。</p><p>4. ページをダブルクリックすると、拡大表示して文字コメントを編集できます。</p><p>5. 「書き出す」でPDF・Word・Excelとして保存します。Word・Excelへの出力はページ画像です。</p><p>「作業を保存」は元PDFを含む編集用ファイルです。完成したPDFの書き出しとは別です。</p><p>Word：${office.word ? "利用可能" : "未検出"} ／ Excel：${office.excel ? "利用可能" : "未検出"}</p><p>パスワード付きPDFの作業コピーは復号された状態で作業ファイル・復旧データに含まれます。</p></div>`,
      );
      break;
  }
}
shell();
render();
if (isTauri()) {
  await listen<{ message: string; percent: number }>("job-progress", (e) => {
    $("#busy-message").textContent = e.payload.message;
    $<HTMLProgressElement>("#progress").value = e.payload.percent;
  });
  await listen<string>("recovery-warning", (e) =>
    toast(`復旧用保存に失敗しました：${e.payload}`, true),
  );
  await getCurrentWindow().onDragDropEvent((e) => {
    if (e.payload.type === "drop" && !busy) void importPaths(e.payload.paths);
  });
  await getCurrentWindow().onCloseRequested(async (e) => {
    if (busy) {
      e.preventDefault();
      toast("処理が終わるかキャンセルしてから閉じてください。", true);
      return;
    }
    if (state.dirty) {
      e.preventDefault();
      if (
        await confirm(
          state.recoveryError
            ? "復旧用保存に失敗しています。終了すると変更を失う可能性があります。終了しますか？"
            : "変更は復旧用に保存されています。終了しますか？",
          { title: "終了" },
        )
      ) {
        await getCurrentWindow().destroy();
      }
    }
  });
  try {
    office = await call({ op: "detect" });
    const button = document.querySelector<HTMLButtonElement>(
      '[data-action="office"]',
    )!;
    button.disabled = !office.word && !office.excel;
    button.title = `Word ${office.word ? "利用可能" : "未検出"} / Excel ${office.excel ? "利用可能" : "未検出"}`;
    if (await call<boolean>({ op: "recovery_exists" })) {
      dialog(
        "作業の復元",
        '<p>前回の作業の復旧データがあります。</p><div class="dialog-actions"><button id="discard-recovery">破棄する</button><button id="recover-project" class="primary">復元する</button></div>',
      );
      $("#recover-project").onclick = async () => {
        if (await change({ op: "recover" })) closeDialog();
      };
      $("#discard-recovery").onclick = async () => {
        await call({ op: "discard_recovery" });
        closeDialog();
      };
    }
  } catch (err) {
    toast(String(err), true);
  }
} else {
  toast(
    "画面プレビューです。ファイル操作はデスクトップアプリから利用できます。",
  );
}
