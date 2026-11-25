import { useEffect, useRef, useState } from "react";
import { Maximize2, Minus, Plus, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const MIN_SCALE = 0.25;
const MAX_SCALE = 6;

function clamp(value: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, value));
}

export function ImageZoomPane({ src, alt }: { src: string; alt: string }) {
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [fit, setFit] = useState(true);
  const [dragging, setDragging] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const dragRef = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    originX: number;
    originY: number;
  } | null>(null);

  useEffect(() => {
    setScale(1);
    setOffset({ x: 0, y: 0 });
    setFit(true);
  }, [src]);

  useEffect(() => {
    const node = containerRef.current;
    if (!node) return;
    function onWheel(event: WheelEvent) {
      event.preventDefault();
      const step = event.deltaY < 0 ? 0.25 : -0.25;
      setFit(false);
      setScale((current) => clamp(current + step));
    }
    node.addEventListener("wheel", onWheel, { passive: false });
    return () => node.removeEventListener("wheel", onWheel);
  }, []);

  function reset() {
    setScale(1);
    setOffset({ x: 0, y: 0 });
    setFit(true);
  }

  function jumpTo(value: number) {
    setFit(false);
    setScale(clamp(value));
    setOffset({ x: 0, y: 0 });
  }

  function toggleFit() {
    if (fit) {
      jumpTo(2);
    } else {
      reset();
    }
  }

  function onPointerDown(event: React.PointerEvent<HTMLDivElement>) {
    if (fit) return;
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: offset.x,
      originY: offset.y,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragging(true);
  }

  function onPointerMove(event: React.PointerEvent<HTMLDivElement>) {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    setOffset({
      x: drag.originX + (event.clientX - drag.startX),
      y: drag.originY + (event.clientY - drag.startY),
    });
  }

  function endDrag(event: React.PointerEvent<HTMLDivElement>) {
    if (dragRef.current?.pointerId !== event.pointerId) return;
    dragRef.current = null;
    setDragging(false);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  return (
    <div className="flex min-h-[320px] flex-1 flex-col gap-2">
      <div
        ref={containerRef}
        role="group"
        aria-label={`Visualizador de ${alt}`}
        className={cn(
          "relative flex min-h-[320px] flex-1 select-none touch-none items-center justify-center overflow-hidden rounded-lg border border-zinc-800/70 bg-[repeating-conic-gradient(#18181b_0%_25%,#111113_0%_50%)] bg-[length:24px_24px]",
          fit && "cursor-default",
          !fit && (dragging ? "cursor-grabbing" : "cursor-grab"),
        )}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onDoubleClick={toggleFit}
      >
        <img
          src={src}
          alt={alt}
          draggable={false}
          onDragStart={(event) => event.preventDefault()}
          style={
            fit
              ? undefined
              : {
                  transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
                  transformOrigin: "center center",
                }
          }
          className={cn(
            "select-none",
            fit ? "max-h-full max-w-full object-contain" : "max-w-none",
          )}
        />
      </div>

      <div className="flex flex-wrap items-center gap-1">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() => jumpTo(scale - 0.25)}
          disabled={scale <= MIN_SCALE}
          aria-label="Diminuir zoom"
          title="Diminuir zoom"
        >
          <Minus size={13} />
        </Button>
        <span className="w-12 text-center font-mono text-[12px] text-zinc-400">
          {Math.round(scale * 100)}%
        </span>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() => jumpTo(scale + 0.25)}
          disabled={scale >= MAX_SCALE}
          aria-label="Aumentar zoom"
          title="Aumentar zoom"
        >
          <Plus size={13} />
        </Button>
        <Button
          variant={fit ? "secondary" : "ghost"}
          size="sm"
          className="h-7 px-2 text-[12px]"
          onClick={reset}
        >
          <Maximize2 size={12} />
          Ajustar
        </Button>
        <Button
          variant={!fit && scale === 1 ? "secondary" : "ghost"}
          size="sm"
          className="h-7 px-2 text-[12px]"
          onClick={() => jumpTo(1)}
        >
          100%
        </Button>
        <Button
          variant={!fit && scale === 2 ? "secondary" : "ghost"}
          size="sm"
          className="h-7 px-2 text-[12px]"
          onClick={() => jumpTo(2)}
        >
          200%
        </Button>
        <span className="ml-auto hidden items-center gap-1 text-[11.5px] text-zinc-500 sm:flex">
          <RotateCcw size={11} />
          roda, arrasta ou clique duplo
        </span>
      </div>
    </div>
  );
}
