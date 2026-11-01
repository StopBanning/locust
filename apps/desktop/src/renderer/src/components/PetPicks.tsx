import { useEffect, useRef, useState } from 'react'
import type { ReactElement } from 'react'

import type { PetRef } from '../../../shared/avatar.js'
import type { PublicPet } from '../../../shared/ipc.js'
import { PET_ADD_CHANNEL, PET_REMOVE_CHANNEL } from '../../../shared/ipc.js'
import { failureSaid, onTheComputer } from '../onTheComputer.js'
import { OPENPETS_LINK, OPENPETS_REPORT_LINK } from '../../../shared/outbound-links.js'
import { PET_PICKS } from '../../../shared/pet-picks.js'
import type { PetPick } from '../../../shared/pet-picks.js'
import { refreshPetList, setPetLook } from '../pets.js'

/**
 * THE PETS LOCUST OFFERS, AS TILES IN THE LOOK PICKER (0.564).
 *
 * 0.563 browsed openpets.dev's whole gallery here: search, Featured or
 * Originals, a page of tiles and More. Colin kept the 21 he had added from it
 * and asked for *"all the others"* to go (shared/pet-picks.ts), so the Pets
 * row is those 21, always there: no search, no paging. A tile shows the pet's
 * small picture -- light to show, where 21 live sheets were seconds of
 * decoding. A pet not on this computer yet downloads from openpets.dev on its
 * click and goes on the teammate; the credit line says so once (0.565: an
 * arrow on each tile put 21 arrows on a new computer's row).
 * Nothing is downloaded before a click, and each pet's rights stay with its
 * maker (PetCredit says so, and where to report one).
 */

/** Small pictures already read this session, by pet id. */
const thumbnails = new Map<string, string>()

function PickPicture({ id }: { readonly id: string }): ReactElement {
  const ref = useRef<HTMLSpanElement>(null)
  const [source, setSource] = useState(thumbnails.get(id))
  useEffect(() => {
    if (source !== undefined) return undefined
    const element = ref.current
    const bridge = window.desktop
    if (element === null || bridge === undefined) return undefined
    let cancelled = false
    const read = (): void => {
      void bridge.petThumbnail(id).then((answer) => {
        if (!answer.ok || cancelled) return
        thumbnails.set(id, answer.data.dataUrl)
        setSource(answer.data.dataUrl)
      })
    }
    // Read when it comes into view: the dialog may open scrolled away from the pets.
    if (typeof IntersectionObserver === 'undefined') {
      read()
      return () => {
        cancelled = true
      }
    }
    const watch = new IntersectionObserver(([entry]) => {
      if (entry?.isIntersecting !== true) return
      watch.disconnect()
      read()
    })
    watch.observe(element)
    return () => {
      cancelled = true
      watch.disconnect()
    }
  }, [id, source])
  return (
    <span ref={ref} className="lc-pettile__picture" aria-hidden>
      {source !== undefined ? <img src={source} alt="" draggable={false} /> : <span className="lc-pettile__placeholder" />}
    </span>
  )
}

/**
 * TAKING PETS OUT OF THE PICKER, SEVERAL AT ONCE (0.569).
 *
 * Colin, 2026-10-03: "some of them are too janky to even add more animations
 * too, give me an option to delete some of them for now so i dont have to
 * name them individually". Remove some... turns the tiles into ticks; Remove
 * N takes the ticked ones out of the picker and off this computer -- except
 * a pet a teammate wears, whose files stay so their face does too. Show
 * removed lists them again, and picking one brings it back.
 */
export interface PetRemoval {
  /** The ticked picks while choosing; undefined when not choosing. */
  readonly ticked: ReadonlySet<string> | undefined
  readonly busy: boolean
  readonly showRemoved: boolean
  /** What the last removal did: plain when it all went, amber when some could not be removed. */
  readonly report: { readonly text: string; readonly warn: boolean } | undefined
  /** Clears the report: a pet picked after removing says nothing more about the removal (Sonnet's 0.569 pass). */
  readonly dismiss: () => void
  readonly start: () => void
  readonly cancel: () => void
  readonly toggle: (id: string) => void
  readonly setShowRemoved: (show: boolean) => void
  readonly confirm: () => Promise<void>
}

const nameOfPick = (id: string): string => PET_PICKS.find((pick) => pick.id === id)?.displayName ?? id
const listed = (names: readonly string[]): string =>
  names.length <= 1 ? (names[0] ?? '') : `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]!}`

export function usePetRemoval(onNotice: (notice: string | undefined) => void): PetRemoval {
  const [ticked, setTicked] = useState<ReadonlySet<string>>()
  const [busy, setBusy] = useState(false)
  const [showRemoved, setShowRemoved] = useState(false)
  const [report, setReport] = useState<PetRemoval['report']>()
  return {
    ticked,
    busy,
    showRemoved,
    report,
    dismiss: () => setReport(undefined),
    start: () => {
      onNotice(undefined)
      setReport(undefined)
      setShowRemoved(false)
      setTicked(new Set())
    },
    cancel: () => setTicked(undefined),
    toggle: (id) =>
      setTicked((current) => {
        if (current === undefined) return current
        const next = new Set(current)
        if (next.has(id)) next.delete(id)
        else next.add(id)
        return next
      }),
    setShowRemoved,
    confirm: async () => {
      const bridge = window.desktop
      if (ticked === undefined || ticked.size === 0 || bridge === undefined || busy) return
      setBusy(true)
      const removed: string[] = []
      const failed: string[] = []
      const kept = new Map<string, readonly string[]>()
      // One at a time: each is its own folder and its own line in the removed list.
      for (const id of ticked) {
        const answer = await bridge.removePet(id).catch(() => undefined)
        if (answer?.ok !== true) {
          failed.push(nameOfPick(id))
          continue
        }
        removed.push(nameOfPick(id))
        if ((answer.data?.keptFor.length ?? 0) > 0) kept.set(nameOfPick(id), answer.data!.keptFor)
      }
      await refreshPetList()
      setBusy(false)
      setTicked(undefined)
      const said: string[] = []
      if (removed.length > 0) said.push(`Removed ${String(removed.length)} ${removed.length === 1 ? 'pet' : 'pets'} from the picker; Show removed brings any back.`)
      for (const [pet, wearers] of kept) said.push(`${pet} stays on ${listed(wearers)} until ${wearers.length === 1 ? 'they get' : 'each gets'} another look.`)
      if (failed.length > 0) said.push(`${listed(failed)} could not be removed and ${failed.length === 1 ? 'is' : 'are'} still in the picker.`)
      setReport({ text: said.join(' '), warn: failed.length > 0 })
    }
  }
}

/** Remove some... / Remove N, Cancel: beside the Pets heading. */
export function PetRemovalControls({ removal }: { readonly removal: PetRemoval }): ReactElement {
  if (removal.ticked === undefined) {
    // A pet's files are the computer's to remove (0.850): in a browser the button is held, and says so.
    const stays = onTheComputer(PET_REMOVE_CHANNEL)
    return (
      <button type="button" className="lc-linkbutton lc-pets__action" onClick={removal.start} disabled={stays !== undefined} {...(stays === undefined ? {} : { title: stays })}>
        Remove some…
      </button>
    )
  }
  const count = removal.ticked.size
  return (
    <span className="lc-pets__actions">
      <button type="button" className="lc-linkbutton lc-pets__action" onClick={removal.cancel} disabled={removal.busy}>
        Cancel
      </button>
      <button
        type="button"
        className="lc-linkbutton lc-pets__action lc-pets__action--remove"
        disabled={count === 0 || removal.busy}
        onClick={() => void removal.confirm()}
      >
        {removal.busy ? 'Removing…' : count === 0 ? 'Tick the pets to remove' : `Remove ${String(count)}`}
      </button>
    </span>
  )
}

export function PetPickTiles({
  selected,
  installed,
  onWear,
  onNotice,
  removed = [],
  removal
}: {
  /** The pet the teammate wears now, marked among the tiles. */
  readonly selected: PetRef | undefined
  /** The pets on this computer, to know which picks need a download. */
  readonly installed: readonly PublicPet[] | undefined
  /** A pick on this computer (now), chosen. */
  readonly onWear: (pet: PublicPet) => void
  /** What went wrong, in words; undefined clears it. */
  readonly onNotice: (notice: string | undefined) => void
  /** The picks taken out of the picker (0.569). */
  readonly removed?: readonly string[]
  /** Choosing pets to remove, and whether the removed are shown. */
  readonly removal?: PetRemoval
}): ReactElement {
  const [adding, setAdding] = useState<string>()

  /** `gone`: a removed pick, picked again -- added again, so it is back in the picker even when its files stayed. */
  const wear = (pick: PetPick, gone = false): void => {
    const here = installed?.find((pet) => pet.source === 'gallery' && pet.id === pick.id)
    if (here !== undefined && !gone) {
      onNotice(undefined)
      onWear(here)
      return
    }
    const bridge = window.desktop
    if (bridge === undefined || adding !== undefined) return
    setAdding(pick.id)
    onNotice(undefined)
    void bridge
      .addPet(pick.id)
      .then(async (answer) => {
        if (!answer.ok) {
          onNotice(answer.error.message)
          return
        }
        // Read afresh: a pet that was missing before is here now.
        setPetLook({ source: 'gallery', id: pick.id }, undefined)
        await refreshPetList()
        onWear(answer.data.pet)
      })
      .catch((error: unknown) => onNotice(failureSaid(error, `${pick.displayName} could not be added. Nothing of it was kept on this computer.`)))
      .finally(() => setAdding(undefined))
  }

  const ticked = removal?.ticked
  const isRemoved = new Set(removed)
  // A pick not on the computer yet is downloaded there (0.850): in a browser its tile is held, and says so.
  const addStays = onTheComputer(PET_ADD_CHANNEL)
  // Removed picks are left out, unless the person asked to see them (and is not choosing more to remove).
  const shown = PET_PICKS.filter((pick) => !isRemoved.has(pick.id) || (removal?.showRemoved === true && ticked === undefined))
  return (
    <>
      {shown.map((pick) => {
        const chosen = selected?.source === 'gallery' && selected.id === pick.id
        const here = installed?.some((pet) => pet.source === 'gallery' && pet.id === pick.id) === true
        const busy = adding === pick.id
        const gone = isRemoved.has(pick.id)
        if (ticked !== undefined) {
          const on = ticked.has(pick.id)
          return (
            <button
              key={pick.id}
              type="button"
              aria-pressed={on}
              aria-label={`Remove ${pick.displayName}`}
              title={on ? `${pick.displayName} will be removed` : `Tick ${pick.displayName} to remove it`}
              data-pet={pick.id}
              data-source="gallery"
              className={`lc-look lc-pettile${on ? ' is-ticked' : ''}`}
              disabled={removal?.busy === true}
              onClick={() => removal?.toggle(pick.id)}
            >
              <PickPicture id={pick.id} />
              {on && <span className="lc-pettile__tick" aria-hidden>×</span>}
            </button>
          )
        }
        return (
          <button
            key={pick.id}
            type="button"
            role="radio"
            aria-checked={chosen}
            aria-label={pick.displayName}
            aria-busy={busy}
            title={addStays !== undefined && (!here || gone) ? `${pick.displayName}. ${addStays}` : gone ? `${pick.displayName}: removed; pick it to bring it back` : here ? pick.displayName : `${pick.displayName}: downloads from openpets.dev when picked`}
            data-pet={pick.id}
            data-source="gallery"
            data-here={here ? 'yes' : 'no'}
            {...(gone ? { 'data-removed': 'yes' } : {})}
            className={`lc-look lc-pettile${chosen ? ' is-selected' : ''}${busy ? ' is-busy' : ''}${gone ? ' is-removed' : ''}`}
            disabled={(adding !== undefined && !busy) || (addStays !== undefined && (!here || gone))}
            onClick={() => wear(pick, gone)}
          >
            <PickPicture id={pick.id} />
          </button>
        )
      })}
    </>
  )
}

/** Where the pets come from, and where to report one. */
export function PetCredit(): ReactElement {
  const [linkNotice, setLinkNotice] = useState<string>()
  // A link the host will not open says so (a-refused-link-says-so.test.ts).
  const open = (url: string): void => {
    setLinkNotice(undefined)
    void window.desktop?.openLink(url).then((answer) => {
      if (!answer.ok) setLinkNotice(answer.message)
    })
  }
  return (
    <>
      <p className="lc-pets__caption lc-pets__credit">
        Pets from{' '}
        <button type="button" className="lc-linkbutton" onClick={() => open(OPENPETS_LINK)}>
          openpets.dev
        </button>
        , made by its community; each downloads the first time it is picked. Rights stay with each pet&rsquo;s maker.{' '}
        <button type="button" className="lc-linkbutton" onClick={() => open(OPENPETS_REPORT_LINK)}>
          Report a pet
        </button>
      </p>
      {linkNotice !== undefined && <p className="lc-pets__caption lc-tone-amber">{linkNotice}</p>}
    </>
  )
}
