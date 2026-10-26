export type FileMenuOpener = (path: string, x: number, y: number) => void

let opener: FileMenuOpener | undefined

/** The window binds this. A right-click on a path calls it. */
export function bindFileMenu(next: FileMenuOpener | undefined): void {
  opener = next
}

export function openFileMenu(path: string, x: number, y: number): void {
  opener?.(path, x, y)
}

export function fileMenuEvent(path: string): (event: { preventDefault(): void; stopPropagation(): void; clientX: number; clientY: number }) => void {
  return (event) => {
    event.preventDefault()
    event.stopPropagation()
    openFileMenu(path, event.clientX, event.clientY)
  }
}
