import { useEffect, useState } from 'react'
import type { ReactElement } from 'react'

import type { SkillLibraryPreview, SkillLibrarySkill, SkillLibrarySource } from '../../../shared/ipc.js'
import { POPULAR_SKILLS, POPULAR_SKILLS_AS_OF, TRENDING_SKILLS, TRENDING_SKILLS_AS_OF, skillPreviewPicks } from '../../../shared/popular-skills.js'
import { SKILL_RUNTIME_NAMES } from '../../../shared/skill-runtimes.js'
import { ArmedButton } from './ArmedButton.js'

/**
 * SKILLS FROM GITHUB, in Settings (0.710, skill-library.ts).
 *
 * Paste a public repository, look, tick the skills to keep, keep. The look
 * lists every file each skill carries, and names the ones that run, before
 * anything is downloaded: a skill is instructions a teammate follows, and in
 * Auto its scripts run without asking. What is kept is the commit that was
 * looked at; "Look for changes" reads the branch again and shows what moved.
 */

export const SKILLS_LOOKING = 'Looking…'

const shortSha = (sha: string): string => sha.slice(0, 7)

export function sizeText(bytes: number): string {
  if (bytes < 1024) return `${String(bytes)} B`
  if (bytes < 1024 * 1024) return `${String(Math.round(bytes / 1024))} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

/** "Oct 24" from 2026-10-24, read as the calendar day it names wherever the computer is. */
export function sinceText(day: string): string {
  const [year, month, date] = day.split('-').map(Number)
  return new Date(year ?? 0, (month ?? 1) - 1, date ?? 1).toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

const plural = (count: number, one: string, many = `${one}s`): string => `${String(count)} ${count === 1 ? one : many}`

/** The files of a skill that run, said in one line, or nothing. */
export function runsText(paths: readonly string[]): string | undefined {
  if (paths.length === 0) return undefined
  return `Runs: ${paths.join(', ')}`
}

function keptDate(iso: string): string {
  const date = new Date(iso)
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleDateString([], { month: 'short', day: 'numeric' })
}

export function SkillsFromGitHub(): ReactElement {
  const [sources, setSources] = useState<readonly SkillLibrarySource[]>()
  const [link, setLink] = useState('')
  const [busy, setBusy] = useState<string>()
  const [said, setSaid] = useState<{ readonly text: string; readonly good: boolean }>()
  const [preview, setPreview] = useState<SkillLibraryPreview>()
  const [picked, setPicked] = useState<ReadonlySet<string>>(new Set())

  const load = (): void => {
    void window.desktop?.listSkillLibrary().then((response) => {
      if (response.ok) setSources(response.data.sources)
    }).catch(() => undefined)
  }
  useEffect(load, [])

  const look = (target: string, all = false): void => {
    if (target.trim().length === 0 || busy !== undefined) return
    setBusy(SKILLS_LOOKING)
    setSaid(undefined)
    setPreview(undefined)
    void window.desktop
      ?.previewSkillLibrary(target.trim())
      .then((response) => {
        if (!response.ok) {
          setSaid({ text: response.error.message, good: false })
          return
        }
        setPreview(response.data)
        // Shelf picks are ready for the second click; pasted links keep their opt-in behavior.
        setPicked(skillPreviewPicks(response.data, all))
      })
      .catch(() => setSaid({ text: 'Locust could not look at that repository.', good: false }))
      .finally(() => setBusy(undefined))
  }

  const keepable = preview?.skills.filter((skill) => skill.held === undefined) ?? []
  const keptHere = preview?.skills.filter((skill) => skill.keptAt !== undefined) ?? []
  // Kept at an older commit than the one just looked at: keeping again moves them.
  const behind = preview !== undefined && keptHere.some((skill) => skill.keptAt !== preview.sha)
  const chosen = keepable.filter((skill) => picked.has(skill.name))
  const chosenRuns = chosen.filter((skill) => skill.files.some((file) => file.runs)).length
  const toggle = (name: string): void =>
    setPicked((held) => {
      const next = new Set(held)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })

  // One tile for both rows: a pack opens the same Look, and Keep stays a separate decision.
  const packTile = (pack: { readonly source: string; readonly name: string; readonly description: string; readonly stars: number; readonly license: string }, meta: string): ReactElement => (
    <div className="lc-skilllib__tile" key={pack.source}>
      <button type="button" className="lc-skilllib__pack" disabled={busy !== undefined} onClick={() => { setLink(pack.source); look(pack.source, true) }}>
        <span className="lc-skilllib__name">{pack.name}</span>
        <span className="lc-skilllib__skills">{pack.description}</span>
        <span className="lc-skilllib__meta">{meta}</span>
      </button>
      <button type="button" className="lc-linkbutton" onClick={() => { void window.desktop?.openLink(`https://github.com/${pack.source}`).then((answer) => setSaid(answer.ok ? undefined : { text: answer.message, good: false })) }}>Repository: {pack.source}</button>
    </div>
  )

  return (
    <div className="lc-skilllib">
      <section aria-label="Popular skills" className="lc-skilllib__popular">
        <div className="lc-skilllib__name">Popular skills</div>
        <p className="lc-skilllib__said">Look at a pack to see its skills and files that run, then keep them all. Stars as of {POPULAR_SKILLS_AS_OF}.</p>
        <div className="lc-skilllib__tiles">
          {POPULAR_SKILLS.map((pack) => packTile(pack, `${pack.stars.toLocaleString('en-US')} stars · ${pack.license}`))}
        </div>
        <div className="lc-skilllib__name">New this week</div>
        <p className="lc-skilllib__said">Packs made in the last two weeks that climbed fastest. Stars as of {TRENDING_SKILLS_AS_OF}.</p>
        <div className="lc-skilllib__tiles">
          {TRENDING_SKILLS.map((pack) => packTile(pack, `${pack.stars.toLocaleString('en-US')} stars since ${sinceText(pack.since)} · ${pack.license}`))}
        </div>
      </section>
      {sources !== undefined && sources.length > 0 && (
        <div className="lc-settingrows">
          {sources.map((kept) => {
            const runs = kept.skills.reduce((sum, skill) => sum + skill.runs.length, 0)
            return (
              <div className="lc-settingrow lc-skilllib__kept" key={kept.source}>
                <span className="lc-skilllib__text">
                  <span className="lc-skilllib__name">{kept.source}</span>
                  <span className="lc-skilllib__meta lc-mono">
                    {kept.ref === kept.sha ? shortSha(kept.sha) : `${kept.ref} @ ${shortSha(kept.sha)}`} · kept {keptDate(kept.keptAt)}
                    {' · '}
                    {plural(kept.skills.length, 'skill')}
                    {runs > 0 ? ` · ${plural(runs, 'file')} that run` : ''}
                  </span>
                  <span className="lc-skilllib__skills">{kept.skills.map((skill) => skill.name).join(', ')}</span>
                </span>
                <button
                  type="button"
                  className="lc-button"
                  disabled={busy !== undefined}
                  onClick={() => {
                    const target = kept.ref === kept.sha ? kept.source : `https://github.com/${kept.source}/tree/${encodeURIComponent(kept.ref)}`
                    setLink(kept.source)
                    look(target)
                  }}
                >
                  Look for changes
                </button>
                <ArmedButton
                  className="lc-button"
                  ariaLabel={`Remove the skills kept from ${kept.source}`}
                  armedLabel={`Remove ${plural(kept.skills.length, 'skill')}?`}
                  onConfirm={() => {
                    void window.desktop?.removeSkillLibrary(kept.source).then((response) => {
                      if (response.ok) setSources(response.data.sources)
                      if (preview?.source === kept.source) setPreview(undefined)
                      setSaid({ text: `Removed the skills kept from ${kept.source}. Runs that start now do not get them.`, good: true })
                    })
                  }}
                >
                  Remove
                </ArmedButton>
              </div>
            )
          })}
        </div>
      )}

      <form
        className="lc-skilllib__look"
        onSubmit={(event) => {
          event.preventDefault()
          look(link)
        }}
      >
        <input
          className="lc-input lc-mono"
          value={link}
          maxLength={500}
          placeholder="owner/repo, or its github.com link"
          aria-label="A GitHub repository with skills"
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setLink(event.target.value)}
        />
        <button type="submit" className="lc-button" disabled={link.trim().length === 0 || busy !== undefined}>
          {busy === SKILLS_LOOKING ? SKILLS_LOOKING : 'Look'}
        </button>
      </form>

      {said !== undefined && (
        <p className={`lc-skilllib__said${said.good ? '' : ' lc-tone-amber'}`} role="status">
          {said.text}
        </p>
      )}

      {preview !== undefined && (
        <div className="lc-skilllib__preview" aria-label={`Skills in ${preview.source}`}>
          <div className="lc-skilllib__previewhead">
            <span className="lc-skilllib__name">{preview.source}</span>
            <span className="lc-skilllib__meta lc-mono">
              {preview.ref === preview.sha ? shortSha(preview.sha) : `${preview.ref} @ ${shortSha(preview.sha)}`} · {plural(preview.skills.length, 'skill')}
            </span>
            {keepable.length > 1 && (
              <button
                type="button"
                className="lc-linkbutton"
                onClick={() => setPicked(chosen.length === keepable.length ? new Set() : new Set(keepable.map((skill) => skill.name)))}
              >
                {chosen.length === keepable.length ? 'Pick none' : 'Pick all'}
              </button>
            )}
          </div>
          {keptHere.length > 0 && (
            <p className="lc-skilllib__said">
              {behind
                ? `Changed since you kept ${keptHere.length === 1 ? 'it' : 'them'}. Keep again to move to ${shortSha(preview.sha)}.`
                : `Up to date: nothing has changed since you kept ${keptHere.length === 1 ? 'it' : 'them'}.`}
            </p>
          )}
          {preview.partial && (
            <p className="lc-skilllib__said lc-tone-amber">GitHub listed only part of this repository. Link a folder inside it to see the rest.</p>
          )}
          <ul className="lc-skilllib__list">
            {preview.skills.map((skill) => (
              <SkillRow key={`${skill.folder}/${skill.name}`} skill={skill} sha={preview.sha} picked={picked.has(skill.name)} onToggle={() => toggle(skill.name)} />
            ))}
          </ul>
          <div className="lc-skilllib__actions">
            {chosenRuns > 0 && (
              <span className="lc-skilllib__warn lc-tone-amber">
                {chosenRuns === 1 ? 'One of these carries' : `${String(chosenRuns)} of these carry`} files that run. In Auto, a teammate can run them without asking.
              </span>
            )}
            <button type="button" className="lc-button" onClick={() => setPreview(undefined)}>
              Cancel
            </button>
            <button
              type="button"
              className="lc-primarybutton"
              disabled={busy !== undefined || chosen.length === 0}
              onClick={() => {
                setBusy('keeping')
                setSaid(undefined)
                void window.desktop
                  ?.installSkillLibrary({ source: preview.source, ref: preview.ref, sha: preview.sha, names: chosen.map((skill) => skill.name), ...(preview.path === undefined ? {} : { path: preview.path }) })
                  .then((response) => {
                    if (!response.ok) {
                      setSaid({ text: response.error.message, good: false })
                      return
                    }
                    setSources(response.data.sources)
                    setPreview(undefined)
                    setLink('')
                    setSaid({
                      text: `Kept ${plural(chosen.length, 'skill')} from ${preview.source}. ${SKILL_RUNTIME_NAMES} teammates get ${chosen.length === 1 ? 'it' : 'them'} from their next run.`,
                      good: true
                    })
                  })
                  .catch(() => setSaid({ text: 'Nothing was kept. Try again.', good: false }))
                  .finally(() => setBusy(undefined))
              }}
            >
              {busy === 'keeping' ? 'Keeping…' : behind ? `Keep ${plural(chosen.length, 'skill')} at ${shortSha(preview.sha)}` : `Keep ${plural(chosen.length, 'skill')}`}
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

function SkillRow({ skill, sha, picked, onToggle }: { readonly skill: SkillLibrarySkill; readonly sha: string; readonly picked: boolean; readonly onToggle: () => void }): ReactElement {
  const runs = runsText(skill.files.filter((file) => file.runs).map((file) => file.path))
  const changed = skill.keptAt !== undefined && skill.keptAt !== sha
  return (
    <li className={`lc-skilllib__skill${skill.held === undefined ? '' : ' is-held'}`}>
      <label className="lc-skilllib__pick">
        <input type="checkbox" checked={picked && skill.held === undefined} disabled={skill.held !== undefined} onChange={onToggle} />
        <span className="lc-skilllib__skilltext">
          <span className="lc-skilllib__skillname">
            {skill.name}
            {skill.keptAt !== undefined && <span className="lc-tag">{changed ? `kept at ${shortSha(skill.keptAt)}` : 'kept'}</span>}
          </span>
          {skill.description !== undefined && <span className="lc-skilllib__desc">{skill.description}</span>}
          <span className="lc-skilllib__meta lc-mono">
            {skill.folder === '' ? '(top of the repository)' : skill.folder} · {plural(skill.files.length, 'file')} · {sizeText(skill.bytes)}
          </span>
          {runs !== undefined && <span className="lc-skilllib__runs lc-mono lc-tone-amber">{runs}</span>}
          {skill.held !== undefined && <span className="lc-skilllib__held">{skill.held}</span>}
        </span>
      </label>
    </li>
  )
}
