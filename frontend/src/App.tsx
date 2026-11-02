
import { useEffect, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import "./App.css";

type ElementKind = "text" | "shape" | "image" | "icon";

type SlideElement = {
  id: string;
  kind: ElementKind;
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;
  zIndex: number;
  locked: boolean;
  opacity: number;
  content: Record<string, unknown>;
  style: Record<string, unknown>;
};

type SlideDesign = {
  slideId: string;
  width: number;
  height: number;
  background: string;
  elements: SlideElement[];
};

const SLIDE_ID = "370cd9ff-4004-4487-8d74-05dd080b1c7b";
const API = `/api/slides/${SLIDE_ID}/design`;

const initialDesign: SlideDesign = {
  slideId: SLIDE_ID,
  width: 1920,
  height: 1080,
  background: "#101827",
  elements: [],
};

const numberValue = (value: unknown, fallback: number) =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback;

const stringValue = (value: unknown, fallback: string) =>
  typeof value === "string" ? value : fallback;

export default function App() {
  const [design, setDesign] = useState<SlideDesign>(initialDesign);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [message, setMessage] = useState("");
  const [zoom, setZoom] = useState(0.5);

  const canvasRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{
    id: string;
    startX: number;
    startY: number;
    originalX: number;
    originalY: number;
  } | null>(null);

  const selected = design.elements.find(el => el.id === selectedId);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const response = await fetch(API);
        if (!response.ok) {
          throw new Error(`Loading failed: HTTP ${response.status}`);
        }
        const data: SlideDesign = await response.json();
        if (!cancelled) {
          setDesign(data);
          setMessage("Slide loaded from SQLite");
        }
      } catch (error) {
        if (!cancelled) {
          setMessage(error instanceof Error ? error.message : "Load failed");
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => { cancelled = true; };
  }, []);

  function updateDesign(updater: (current: SlideDesign) => SlideDesign) {
    setDesign(updater);
    setDirty(true);
  }

  function updateElement(id: string, changes: Partial<SlideElement>) {
    updateDesign(current => ({
      ...current,
      elements: current.elements.map(el =>
        el.id === id ? { ...el, ...changes } : el
      ),
    }));
  }

  function updateContent(id: string, key: string, value: unknown) {
    updateDesign(current => ({
      ...current,
      elements: current.elements.map(el =>
        el.id === id
          ? { ...el, content: { ...el.content, [key]: value } }
          : el
      ),
    }));
  }

  function updateStyle(id: string, key: string, value: unknown) {
    updateDesign(current => ({
      ...current,
      elements: current.elements.map(el =>
        el.id === id
          ? { ...el, style: { ...el.style, [key]: value } }
          : el
      ),
    }));
  }

  function addElement(kind: ElementKind) {
    const id = crypto.randomUUID();

    const element: SlideElement = {
      id,
      kind,
      x: 180,
      y: 180,
      width: kind === "text" ? 760 : 360,
      height: kind === "text" ? 130 : 220,
      rotation: 0,
      zIndex: Math.max(0, ...design.elements.map(el => el.zIndex)) + 1,
      locked: false,
      opacity: 1,
      content:
        kind === "text" ? { text: "Your new text" } :
        kind === "shape" ? { shape: "rectangle" } :
        kind === "image" ? { src: "", alt: "Slide image" } :
        { text: "★" },
      style: {
        color: "#FFFFFF",
        backgroundColor: kind === "shape" ? "#00D4FF" : "transparent",
        fontFamily: "Inter, Arial, sans-serif",
        fontSize: kind === "text" ? 64 : 100,
        fontWeight: 700,
      },
    };

    updateDesign(current => ({
      ...current,
      elements: [...current.elements, element],
    }));
    setSelectedId(id);
  }

  function deleteSelected() {
    if (!selectedId) return;
    updateDesign(current => ({
      ...current,
      elements: current.elements.filter(el => el.id !== selectedId),
    }));
    setSelectedId(null);
  }

  function duplicateSelected() {
    if (!selected) return;
    const copy: SlideElement = {
      ...selected,
      id: crypto.randomUUID(),
      x: selected.x + 40,
      y: selected.y + 40,
      content: { ...selected.content },
      style: { ...selected.style },
      zIndex: Math.max(...design.elements.map(el => el.zIndex)) + 1,
    };
    updateDesign(current => ({
      ...current,
      elements: [...current.elements, copy],
    }));
    setSelectedId(copy.id);
  }

  function startDrag(event: ReactPointerEvent<HTMLDivElement>, el: SlideElement) {
    event.stopPropagation();
    setSelectedId(el.id);
    if (el.locked || event.button !== 0) return;

    dragRef.current = {
      id: el.id,
      startX: event.clientX,
      startY: event.clientY,
      originalX: el.x,
      originalY: el.y,
    };

    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function moveDrag(event: ReactPointerEvent<HTMLDivElement>) {
    const drag = dragRef.current;
    if (!drag) return;

    const x = Math.round(drag.originalX + (event.clientX - drag.startX) / zoom);
    const y = Math.round(drag.originalY + (event.clientY - drag.startY) / zoom);

    updateElement(drag.id, { x, y });
  }

  function endDrag() {
    dragRef.current = null;
  }

  async function save() {
    setSaving(true);
    setMessage("Saving...");

    try {
      const { slideId: _slideId, ...payload } = design;
      void _slideId;

      const response = await fetch(API, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      if (!response.ok) {
        throw new Error(`Save failed: HTTP ${response.status}`);
      }

      setMessage("Saved to SQLite successfully");
      setDirty(false);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Save failed");
    } finally {
      setSaving(false);
    }
  }

  function elementStyle(el: SlideElement): CSSProperties {
    return {
      position: "absolute",
      left: el.x,
      top: el.y,
      width: el.width,
      height: el.height,
      transform: `rotate(${el.rotation}deg)`,
      zIndex: el.zIndex,
      opacity: el.opacity,
      fontFamily: stringValue(el.style.fontFamily, "Inter, Arial, sans-serif"),
      fontSize: numberValue(el.style.fontSize, 64),
      fontWeight: numberValue(el.style.fontWeight, 700),
      color: stringValue(el.style.color, "#FFFFFF"),
      backgroundColor: stringValue(el.style.backgroundColor, "transparent"),
      borderRadius: numberValue(el.style.borderRadius, 0),
      cursor: el.locked ? "not-allowed" : "move",
      overflow: "hidden",
      userSelect: "none",
      boxSizing: "border-box",
    };
  }

  function renderElement(el: SlideElement) {
    if (el.kind === "image") {
      const src = stringValue(el.content.src, "");
      return src ? (
        <img
          src={src}
          alt={stringValue(el.content.alt, "")}
          draggable={false}
          style={{ width: "100%", height: "100%", objectFit: "cover" }}
        />
      ) : <div className="image-placeholder">Add an image URL →</div>;
    }

    if (el.kind === "shape") return null;

    return (
      <div style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>
        {stringValue(el.content.text, "")}
      </div>
    );
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">◈</span> OctopilotSlides</div>
        <div className="document-name">Introduction to Cybersecurity</div>
        <div className="top-actions">
          <span className="status">{dirty ? "● Unsaved changes" : message}</span>
          <button className="primary" onClick={save} disabled={saving || loading}>
            {saving ? "Saving..." : "Save presentation"}
          </button>
        </div>
      </header>

      <div className="workspace">
        <aside className="left-panel">
          <div className="panel-heading">INSERT ELEMENTS</div>
          <button onClick={() => addElement("text")}>T &nbsp; Text</button>
          <button onClick={() => addElement("shape")}>▭ &nbsp; Shape</button>
          <button onClick={() => addElement("image")}>▧ &nbsp; Image</button>
          <button onClick={() => addElement("icon")}>★ &nbsp; Icon</button>

          <div className="panel-heading second-heading">LAYERS</div>
          <div className="layers">
            {[...design.elements]
              .sort((a, b) => b.zIndex - a.zIndex)
              .map(el => (
                <button
                  key={el.id}
                  className={selectedId === el.id ? "layer active" : "layer"}
                  onClick={() => setSelectedId(el.id)}
                >
                  <span>{el.kind === "text" ? "T" : el.kind === "shape" ? "▭" : "▧"}</span>
                  <span className="layer-name">
                    {el.kind === "text"
                      ? stringValue(el.content.text, "Text")
                      : el.kind}
                  </span>
                  {el.locked ? "🔒" : ""}
                </button>
              ))}
          </div>
        </aside>

        <main className="editor">
          <div className="editor-toolbar">
            <span>SLIDE 01</span>
            <div className="zoom-controls">
              <button onClick={() => setZoom(z => Math.max(0.2, +(z - 0.1).toFixed(2)))}>−</button>
              <span>{Math.round(zoom * 100)}%</span>
              <button onClick={() => setZoom(z => Math.min(0.8, +(z + 0.1).toFixed(2)))}>+</button>
            </div>
          </div>

          <div className="canvas-viewport">
            <div
              className="canvas-frame"
              style={{
                width: design.width * zoom,
                height: design.height * zoom,
              }}
            >
              <div
                ref={canvasRef}
                className="slide-canvas"
                onPointerDown={() => setSelectedId(null)}
                style={{
                  width: design.width,
                  height: design.height,
                  background: design.background,
                  transform: `scale(${zoom})`,
                }}
              >
                {[...design.elements]
                  .sort((a, b) => a.zIndex - b.zIndex)
                  .map(el => (
                    <div
                      key={el.id}
                      className={`slide-element ${selectedId === el.id ? "selected" : ""}`}
                      style={elementStyle(el)}
                      onPointerDown={event => startDrag(event, el)}
                      onPointerMove={moveDrag}
                      onPointerUp={endDrag}
                      onPointerCancel={endDrag}
                    >
                      {renderElement(el)}
                    </div>
                  ))}
              </div>
            </div>
            {loading && <div className="loading">Loading slide...</div>}
          </div>
          <div className="editor-footer">
            <span>Editable HTML/CSS elements</span>
            <span>{design.elements.length} elements</span>
          </div>
        </main>

        <aside className="right-panel">
          <div className="panel-heading">PROPERTIES</div>
          {!selected ? (
            <div className="properties">
              <p className="muted">Select an element to edit its properties.</p>
              <label>Slide background</label>
              <input
                type="color"
                value={/^#[0-9a-fA-F]{6}$/.test(design.background) ? design.background : "#101827"}
                onChange={e => updateDesign(d => ({ ...d, background: e.target.value }))}
              />
              <label>Canvas</label>
              <p className="muted">{design.width} × {design.height}</p>
            </div>
          ) : (
            <div className="properties">
              <div className="selected-kind">{selected.kind.toUpperCase()}</div>

              {(selected.kind === "text" || selected.kind === "icon") && (
                <>
                  <label>Text</label>
                  <textarea
                    value={stringValue(selected.content.text, "")}
                    onChange={e => updateContent(selected.id, "text", e.target.value)}
                    rows={4}
                  />
                  <label>Font size</label>
                  <input
                    type="number"
                    min="1"
                    value={numberValue(selected.style.fontSize, 64)}
                    onChange={e => updateStyle(selected.id, "fontSize", +e.target.value)}
                  />
                  <label>Text color</label>
                  <input
                    type="color"
                    value={stringValue(selected.style.color, "#FFFFFF")}
                    onChange={e => updateStyle(selected.id, "color", e.target.value)}
                  />
                </>
              )}

              {selected.kind === "image" && (
                <>
                  <label>Image URL</label>
                  <input
                    value={stringValue(selected.content.src, "")}
                    onChange={e => updateContent(selected.id, "src", e.target.value)}
                    placeholder="https://..."
                  />
                  <label>Alt text</label>
                  <input
                    value={stringValue(selected.content.alt, "")}
                    onChange={e => updateContent(selected.id, "alt", e.target.value)}
                  />
                </>
              )}

              {selected.kind === "shape" && (
                <>
                  <label>Fill color</label>
                  <input
                    type="color"
                    value={stringValue(selected.style.backgroundColor, "#00D4FF")}
                    onChange={e => updateStyle(selected.id, "backgroundColor", e.target.value)}
                  />
                  <label>Corner radius</label>
                  <input
                    type="number"
                    min="0"
                    value={numberValue(selected.style.borderRadius, 0)}
                    onChange={e => updateStyle(selected.id, "borderRadius", +e.target.value)}
                  />
                </>
              )}

              <div className="property-grid">
                {(["x", "y", "width", "height", "rotation", "zIndex"] as const)
                  .map(key => (
                    <div key={key}>
                      <label>{key}</label>
                      <input
                        type="number"
                        value={selected[key]}
                        onChange={e => updateElement(selected.id, { [key]: +e.target.value })}
                      />
                    </div>
                  ))}
              </div>

              <label>Opacity: {Math.round(selected.opacity * 100)}%</label>
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                value={selected.opacity}
                onChange={e => updateElement(selected.id, { opacity: +e.target.value })}
              />

              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={selected.locked}
                  onChange={e => updateElement(selected.id, { locked: e.target.checked })}
                />
                Lock element
              </label>

              <div className="property-actions">
                <button onClick={duplicateSelected}>Duplicate</button>
                <button className="danger" onClick={deleteSelected}>Delete</button>
              </div>
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}
