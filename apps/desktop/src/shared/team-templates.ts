import type { BotShape, BotFace } from './avatar.js'
import type { TeammateHue, TeammateRole } from './ipc.js'

/**
 * A TEAM IN ONE CLICK.
 *
 * Paperclip's second idea worth taking (2026-09-26): a person who opens
 * Locust for the first time meets an empty Home and one button, and has to
 * invent a teammate -- a name, a role, a face -- before anything happens.
 * Locust is for "quite literally ANY ai user" (Colin): someone writing code,
 * someone watching their money, someone writing or designing. So Home offers
 * a team for each, ready to talk to; every one of them can be renamed,
 * re-faced, given another model or removed like any teammate.
 *
 * No model is chosen here. A template cannot know which of the person's
 * tools are signed in, so each teammate runs on the model the person picks,
 * exactly as a teammate made by hand does before its first run.
 */
export interface TemplateTeammate {
  readonly name: string
  readonly role: TeammateRole
  /** A Custom teammate's own words for what they do. */
  readonly roleTitle?: string
  readonly hue: TeammateHue
  readonly bot: { readonly shape: BotShape; readonly face: BotFace }
  /**
   * What they offer as a first message (0.359), in their team's own world.
   * By role alone, Sable the analyst offered "Summarize what this codebase
   * is for" and Quill the writer "What is the most surprising thing in this
   * codebase?" -- to exactly the people these two teams are for. Each can be
   * sent as it stands: nothing to fill in first.
   */
  readonly starters: readonly string[]
  /**
   * What they offer instead while the folder holds nothing yet (2026-10-30). A new person's first folder is the one
   * Locust made, and it is empty: Wren offered "Read this project" and "Find the riskiest file in this repo" there,
   * and answered, rightly, that there was no project. Absent where the starters already need no files.
   */
  readonly freshStarters?: readonly string[]
}

export interface TeamTemplate {
  readonly templateId: 'software' | 'money' | 'writing'
  readonly title: string
  /** One line: who is on it, said as what they do. */
  readonly line: string
  readonly teammates: readonly TemplateTeammate[]
}

export const TEAM_TEMPLATES: readonly TeamTemplate[] = [
  {
    templateId: 'software',
    title: 'Build software',
    line: 'A coder, a reviewer who tests, and a researcher.',
    teammates: [
      {
        name: 'Wren',
        role: 'Code & Migrations',
        hue: 'lime',
        bot: { shape: 'droid', face: 'eyes' },
        starters: [
          'Read this project and tell me what it does, in one paragraph.',
          'Find the riskiest file in this repo and explain why.',
          'Pick one small thing here that is broken or missing, fix it, and show me what you changed.'
        ],
        freshStarters: [
          'Make a small web page here that shows today\'s date and a quote, and tell me how to open it.',
          'Ask me what I want to build, then set up its first files here.',
          'Write a small script here that renames photos by the date they were taken, and explain how to run it.'
        ]
      },
      {
        name: 'Juno',
        role: 'Docs & QA',
        hue: 'violet',
        bot: { shape: 'cat', face: 'mouth' },
        starters: [
          'What is untested here that would hurt most if it broke?',
          'Find documentation in this repo that no longer matches the code.',
          'Review the most recent change in this repo as if it were a pull request.'
        ],
        freshStarters: [
          'Write me a checklist for testing a website before I share it.',
          'Ask me about a project of mine, then write the README it should have.',
          'Explain what a careful code review looks for, with one short example of each point.'
        ]
      },
      {
        name: 'Atlas',
        role: 'Research & Briefs',
        hue: 'blue',
        bot: { shape: 'hexagon', face: 'eyes' },
        starters: [
          'Summarize what this codebase is for, for someone joining tomorrow.',
          'List the decisions this project has already made that would be expensive to reverse.',
          'Ask Wren what is hardest in this code, then find how other projects solve it, with sources.'
        ],
        freshStarters: [
          'Compare three ways to make a simple website today: what each costs and how hard it is, with sources.',
          'Ask me what I am researching, then write a one-page brief on it, with sources.',
          'Ask Wren what we could build first, then find two or three projects like it, with sources.'
        ]
      }
    ]
  },
  {
    templateId: 'money',
    title: 'Research & money',
    line: 'An analyst, a keeper of the numbers, and a planner.',
    teammates: [
      {
        name: 'Sable',
        role: 'Research & Briefs',
        hue: 'teal',
        bot: { shape: 'alien', face: 'eyes' },
        starters: [
          "Where can I keep savings I won't need for three years? Compare the options with today's rates, and name your sources.",
          'Explain index funds, bonds and CDs in plain words: what each is for, and how each can lose money.',
          'Make me a checklist for judging whether an article about money is worth trusting.'
        ]
      },
      {
        name: 'Penny',
        role: 'Data & Reporting',
        hue: 'butter',
        bot: { shape: 'pill', face: 'mouth' },
        starters: [
          'Make me a monthly budget spreadsheet in this folder, with categories, totals and a line for savings.',
          'If I save $400 a month at 4.5% a year, what will I have in five years? Show the arithmetic.',
          'Read the spreadsheets in this folder and tell me what the numbers say.'
        ]
      },
      {
        name: 'Rook',
        role: 'Chief of Staff',
        hue: 'slate',
        bot: { shape: 'mech', face: 'eyes' },
        starters: [
          'Plan my next three months of saving: ask Sable for the options and Penny for the numbers, then bring me one plan.',
          'Ask Sable and Penny what each would need from me to help with my money, and bring me one list.',
          'I want to pay off a $6,000 card balance within a year. Work out a plan with the team.'
        ]
      }
    ]
  },
  {
    templateId: 'writing',
    title: 'Write & design',
    line: 'A writer, a designer, and an editor.',
    teammates: [
      {
        name: 'Quill',
        role: 'Custom',
        roleTitle: 'Writer',
        hue: 'rose',
        bot: { shape: 'flower', face: 'mouth' },
        starters: [
          'Ask me five questions about my business, then write a one-page introduction to it.',
          'Show me three ways to open a newsletter, each in a different voice, and say when each works.',
          'Turn the notes in this folder into a clear one-page summary.'
        ]
      },
      {
        name: 'Iris',
        role: 'Custom',
        roleTitle: 'Designer',
        hue: 'clay',
        bot: { shape: 'cloud', face: 'eyes' },
        starters: [
          // A brand guide, not a web page: Locust never runs what a teammate
          // wrote (DECISION-2026-09-20), so a page she made would open here as
          // its source code. A guide opens as a document, palette swatched.
          'Make a one-page brand guide for a small bakery in this folder: a colour palette with hex codes, two font pairings, and the voice to write in.',
          'Suggest a colour palette and two font pairings for a calm, modern brand, with hex codes.',
          'Look at the files in this folder and suggest how to make them look more polished.'
        ]
      },
      /*
       * An editor, said as one (0.359). Moss was Docs & QA, whose brief is
       * "match the words to the code, say what is untested" -- the right
       * brief for Juno on the software team and the wrong one for whoever
       * edits a writer's draft.
       */
      {
        name: 'Moss',
        role: 'Custom',
        roleTitle: 'Editor',
        hue: 'pearl',
        bot: { shape: 'pebble', face: 'mouth' },
        starters: [
          'Read the documents in this folder and mark what is unclear, wrong or too long.',
          'Give me a checklist for editing my own writing, with an example for each point.',
          'Ask Quill for a short piece about this team, then edit it and show me both versions.'
        ]
      }
    ]
  }
]

/**
 * The empty-folder starters of the template teammate whose starters these are, if it has any. Matched by the starters
 * a teammate was made with, which a rename keeps; a teammate made by hand has none, and takes its role's (IdleTeammate).
 */
export function freshStartersFor(starters: readonly string[]): readonly string[] | undefined {
  for (const template of TEAM_TEMPLATES) {
    for (const mate of template.teammates) {
      if (mate.starters.length === starters.length && mate.starters.every((one, at) => one === starters[at])) return mate.freshStarters
    }
  }
  return undefined
}
