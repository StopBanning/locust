# Changelog

What changed in each build, written for someone using Locust rather than
reading its source. The commit history carries the engineering detail; this
carries what you would notice.

Dates are when the build was cut. Versions are the number Settings shows.

Settings shows this file as What's new. An entry may group its changes under
`### New`, `### Improved` and `### Fixed`, and a build a person should be told
about when they arrive on it carries `<!-- big -->` on the line under its
heading: the home screen then shows it once, as a splash.

## 0.839.0 - 2026-10-28
<!-- big -->

### New

- **A teammate can have its own browser sign-ins.** Turn on Own browser sign-ins when editing a teammate, and in the browser panel it signs in to sites apart from you and every other teammate, so two teammates can work in two accounts on one site. Its tabs carry its color. To sign it in, press + in the panel and pick its name. Its tools see and use only its own tabs, and no other teammate's reach them. Turning it off, or removing the teammate, forgets those sign-ins.

### Improved

- **Carry on past a provider error, not only a used-up plan.** When a sign-in has expired, a provider is down or busy, or there is no network, and the turn stops before it has done anything, its card now offers Carry on with another agent, which does the task from the start. With an agent picked and Without asking on in Settings > AI agents & accounts, it goes on by itself. A turn that had already used a tool is never moved, so nothing is done twice. The section is now called When an agent cannot go on.

- **Kilo installs from Locust, in one click.** Kilo's row on Home now has Install, as Claude Code, Codex, Copilot and OpenCode have, instead of a link to Kilo's page and npm lines to type into a terminal. Auto Free needs no Kilo account.

- **A working conversation shimmers, as in Claude Code.** In the sidebar, a conversation or room that is working no longer turns a spark beside it: a soft light passes slowly through its title instead, in the window's own greys. With many chats going, the list stays calm. With reduced motion asked for in your system, the title simply stays bright while it works.

### Fixed

- **An agent you install while Locust is open is found when you come back.** After Get it, the next time you return to Locust's window it checks every agent again, the ones it had marked not installed too. Before, it said Kilo was not installed until Locust was restarted.

## 0.838.0 - 2026-10-28
<!-- big -->

### New

- **Your /locust comments are picked up within a minute.** With Scout on, Locust reads your own comments on the folder's GitHub repository every minute instead of waiting for Scout's next look, ten minutes to an hour away. A minute with nothing new asks no AI. A new ask starts Scout's look at once, and its card comes as it always has.
- **A /locust comment reaches your phone while your computer is off.** With Look while my computer is off on, the workflow in your repository also runs when a comment is written. Yours, starting a line with /locust, goes to your phone at once, sealed so the relay cannot read it, and rings it unless it is quiet hours. Anyone else's comment ends the run. When Locust is open again, your ask becomes its card for a teammate. A repository that already has the away look is offered the newer one: Update it, under Look while my computer is off, opens the pull request.
- **Sign in with GitHub to open Locust in any browser, with no code.** In Settings > General > Your phone, choose Sign in with GitHub once. After that, go to app.locust.lol/desk in any browser, on another laptop or a work computer, choose Sign in with GitHub, and the whole window opens, driving your computer. Locust's relay checks who you are with GitHub, then your computer asks GitHub again itself, and lets in only your account. Locust asks GitHub for no permissions, only who you are, and keeps GitHub's token nowhere. Browsers that came in this way say so in the list, and Remove stops one at once. Sign out of GitHub stops new ones.
- **Open in your browser: Locust's whole window in this computer's browser, in one click.** Next to the code in Settings > General > Your phone. With your phone off, it turns on From anywhere first. The browser pairs and opens the full window straight away, with no link to copy and no Full window switch to find.
- **The way in says what Locust is.** A browser not paired yet at app.locust.lol/desk now shows every way in: Sign in with GitHub, a code from your computer, and, for someone who has never used Locust, Download Locust.
- **Plugged in, your computer can stay awake for routines and Scout.** In Settings > General, under Startup, turn on Keep this computer awake while it is plugged in. A routine set for 7:00, or Scout’s next look, used to wait for a computer that went to sleep at 1:00. Only the sleep that comes when nobody uses it is held off: the screen still turns off, Sleep and closing the lid work as always, and on battery it sleeps as usual. Off until you turn it on.

### Fixed

- **Home holds still again.** On a window just short of room, Home flicked between two layouts many times a second: with six teammates from about 880 pixels tall, with three at about 766. Squeezing the spacing to fit also shortened the cards, which made it look as if there was room after all, so it let go, and then had to squeeze again. Home now judges the room at its normal spacing, so once it squeezes it stays put.

## 0.837.0 - 2026-10-27
<!-- big -->

### New

- **Write from your phone while your computer is off.** Paired from anywhere, your phone keeps the conversations it last saw and still shows them with your computer off, saying when it last saw them. A message you write then waits, sealed on Locust's relay so the relay cannot read it, for up to a week, and Locust sends it as your next message when your computer is back. Take back takes it off the relay, and twenty can wait at once. One written more than twelve hours before Locust is back, or whose conversation is gone, is not sent by itself: your phone shows it with Send now, Copy or Discard.
- **What needs you reaches Discord, Slack or Telegram.** Once a chat app is paired, turn on Tell me here when something needs me in its Settings. While you are away from the window, a card waiting on you, from any conversation, comes there with Allow once and Deny, and so do Scout's urgent finds and a teammate that hits its limit, by the same rules as your phone's alerts: merged, a few a day, and held in quiet hours. Always stays in Locust. Each card says what it asks to do and the first line of its file or command. The app's servers can read that, with anything shaped like a key taken out.
- **Claude Code can ask before each action.** Approve each action now runs on Claude Code teammates too: every file change, command and connector call waits on a card for Allow once, Always or Deny before it runs, as it already did on Codex, OpenCode and Copilot.
- **Council review: three AIs check one change.** Routines > Start from a template > Council review hands the changes in your folder not yet committed to three teammates, each proposed on a different AI where your team has one: the first reviews, the second reviews too and agrees or disagrees with each finding, and the third settles them, keeping what at least two would stand behind, and approves only if nothing must change. It changes no file. Who takes each step shows in the editor before you save it. After tstack's council.

### Improved

- **The message box is one row.** As in Claude Code: the + left of where you type, and voice typing, Talk and send right of it. One line is half the height it was. A longer message still grows the box, and the buttons stay on its last line.

### Fixed

- **Codex's commands read as they run on Windows.** An approval card and the Board showed the program's path with every backslash doubled. They now show it the way the shell gets it, and the rest of the command exactly as Codex sent it.
- **A long command wraps at its spaces.** On an approval card and the Board, a command too long for one line could break between a flag's dash and its name, so `-Command` read as `-` and `Command`. A flag now stays whole, and the command copies exactly as sent.

## 0.836.0 - 2026-10-27
<!-- big -->

### New

- **Verbose shows every step.** In a conversation, ··· then Verbose, or Ctrl+O, opens every run's steps and each tool call's detail. Normal folds them again. The choice stays with this window.
- **Right-click a file path.** On a path in an answer, a changed file, a step that names a file, or the file viewer's title, right-click to add it to your next message, open it in VS Code or Cursor when that editor is on your PATH, open a document, picture, sound or video in its usual app (never a file that app would run), show it in Explorer, Finder or the file manager, or copy the path, the path from the folder, or the file name. A path outside the conversation's folder can only be copied.

### Improved

- **The message box is quieter.** The kind of message, the mode, the folder, the usage ring and the model sit under the box as plain words. The model and its effort are one control, such as Haiku 5.5 · Medium, and opening it lists the models with the effort choice at the foot. The send button keeps its look. Comparing models still sets each one's effort on its own.
- **Keyboard shortcuts, in one sheet.** Ctrl+/ lists them, grouped. Ctrl+Shift+M, Ctrl+Shift+I and Ctrl+Shift+E open the mode, model and effort of the conversation on screen. With a menu open, 1 through 9 picks a row. The command palette lists Keyboard shortcuts too.

## 0.835.0 - 2026-10-26
<!-- big -->

### New

- **A routine starts when its private link is called.** In the routine, choose When a link is called. Your phone, from anywhere has to be on. The link is a password: anyone who has it can start that routine, and the routine always runs in Ask, so it reads and changes nothing. What they send passes through Locust's relay readable, unlike sealed phone traffic, and is kept there until your computer collects it, at most a week. Do not send secrets.
- **Kilo is an agent, in Preview.** A new conversation starts on Auto Free, which needs no account. Auto Free may route requests to providers that log prompts and outputs and use them to improve their services. Do not submit personal or confidential data.
- **Compare works from a browser.** Start a comparison, ask again, run it again, have it judged and keep a column, using the models a browser can already run.
- **More settings and branch work from a browser.** Change the look, groups, individual memories and Scout's notes, tune-up and settings, draft and make a commit, and land a teammate's branch or begin its merge. Scout's preparing fixes, the folder's own permissions, forgetting every memory at once, pushing, opening a pull request and throwing a branch away still stay on your computer.
- **Answer a command from the Board.** A conversation in Needs you shows the whole command and where it would run, with Allow once and Deny right there. A file change, connector or question opens the conversation for its full card; Always stays there too.
- **See what a teammate handed out.** Under a turn that sent work to other teammates, Handed out lists who has each ask, its live state and Open to reach its conversation. The full peer exchanges stay beside it.
- **OpenCode teammates can use their kept keys.** Keys you keep for a teammate now reach its OpenCode turns, as they already do for Claude Code and Cursor.

### Improved

- **Account default fits its row.** Its explanation in the model picker now reads "The model your account picks for you" on one line.
- **Muse Code is no longer marked Preview.** Its row in the model picker and Settings reads like every other agent's: it has now finished whole paid runs with dozens of tool calls each, the step its Preview label was waiting on.
- **Changed files are easier to scan.** Each row leads with a mark for its kind and the file's own name, with its folder beside it when two rows share a name and its path in the hover. Long file lists under finished turns show five rows and Show N more, and new, deleted, renamed and large are quiet words beside the name.
- **Home makes more room in a short window.** The illustration shrinks a little and the team cards sit closer together, leaving more room for the chat box and conversation starters. Taller windows keep their existing layout.

### Fixed

- **Codex says when a conversation is open elsewhere.** If the Codex app or a Codex in a terminal already has it open, Locust says to close it there and run it again, instead of showing the whole log.
- **Scout says why GitHub was not read.** What Scout saw distinguishes a folder that is not on GitHub from a signed-out or missing GitHub CLI. Where signing in or installing can fix it, it points to Settings > Connectors > GitHub.
- **Your own agent no longer stops at its first 200 models.** The model picker now keeps up to 1,000 models the agent offers, so a longer list can include models that used to disappear.

## 0.834.0 - 2026-10-26
<!-- big -->

### New

- **Locust in a browser opens at once.** Opened again, app.locust.lol/desk shows straight away the conversations, team and turns it showed last time, under a thin line saying it is what Locust last showed and when, and opens live as soon as your computer answers. With your computer off or the relay out of reach, you can still read what it last showed; nothing can be changed from it until it is live. What the browser keeps is sealed with a key the browser itself holds and will not hand out, and forgetting the pairing forgets it too.
- **Routines, teammates and rooms from a browser.** Locust in a browser now makes, edits, runs, pauses and removes routines, keeps or discards what a routine changed in its copy, makes, edits and removes teammates, and makes rooms, posts to them and moves their board. What opens something on your computer -- choosing a folder, importing or exporting a file -- and what a teammate may spend, its folder, connectors, skills and keys are still changed only in Locust on your computer, and the browser says so.
- **Routines as Markdown files.** Export a routine and you get `Name.routine.md`: what it asks for and where it runs at the top, then its name and each step in its own words, under `## Step 1`, `## Step 2` and so on. Read it, share it, edit it in any editor, and import it again -- Locust reads it with the same checks as before, and still reads the older `.locust-routine.json` files.

### Improved

- **Big token counts read as billions.** Usage now writes 1.1B instead of 1133M, and a count about to read as a thousand of one unit is written as one of the next (1.0B, never 1000M).
- **A key asked for reads as one line.** When a teammate asks for a key, its answer now says "Asks for the key NAME (below)", and the reason it gave is on the card with the box, instead of standing alone in the answer.

## 0.833.0 - 2026-10-24
<!-- big -->

### New

- **Keys a teammate needs.** A teammate that needs a key -- to charge a test card, post to a channel, call a private API -- asks for it by name, and a card under its answer has a masked box and Keep. Or add one yourself in the teammate's settings, under Keys. A key is kept encrypted on your computer for that teammate alone, and only its own turns get it, as an environment variable of that name: never another teammate, a cloud turn or Scout's sealed looks. Its brief names the key, never its value, and if a command prints the value, Locust takes it out of everything it records and shows. Names an agent signs in with, like ANTHROPIC_API_KEY or GITHUB_TOKEN, are refused, so a teammate never pays with a key instead of your plan. Keys reach Claude Code and Cursor teammates for now; other agents follow once each is tested.

### Fixed

- **Antigravity waiting on a stuck helper no longer hides as thinking.** A turn whose helper had stalled showed "Thought for 5m" over and over while each wake-up quietly re-read the whole conversation and used up quota. Now those rows say "Waiting on its helper", with when it last moved; the third time a turn re-reads the whole conversation with nothing reused, Locust says how much quota that has cost and to press Stop; and a model that stops answering for five minutes is said in words.

## 0.832.0 - 2026-10-23
<!-- big -->

### New

- **Your teammates in Discord.** Write to your teammates from Discord, while Locust is open on your computer. Make an application of your own in Discord's Developer Portal, paste its bot's token in Settings > General > Chat apps, add the bot to a server of yours, and send it the eight-digit code Settings shows: that pairs your account. Then type `/` in your messages with the bot and pick a teammate from Discord's own list, or write a name and a colon ("Ada: check the build"); the answer comes back as a reply, with the bot typing while your teammate works. After that, just write, and a reply to an older answer carries that conversation on. A card from a turn you started there comes with Allow once and Deny buttons; Always stays on your computer. The bot reads only its direct messages, so you turn nothing on in the Developer Portal, and only your account is answered.
- **Your teammates in Slack.** The same in Slack. Settings' Create the app in Slack opens Slack with the app already filled in: pick your workspace, install it, and paste its two tokens. Send the Locust app the code Settings shows, then write "Ada: check the build". Your message is marked with 👀 while Ada works, the answer comes back in the chat, and a reply in an answer's thread carries that conversation on in the thread. "who", "new" and "waiting" work as in the other apps, and cards come with Allow once and Deny.

### Improved

- **Chat apps, in one place.** Discord, Slack and Telegram are rows of one Chat apps section in Settings > General, each Off until you press Set up, and each walks you through its steps where they are done. A teammate's name and a colon now writes to it in Telegram too.
- **Chat tokens are never repeated.** A Discord bot's token, a Slack app's token and a Telegram bot's token are now among the shapes Locust takes out of anything sent to a chat app, kept in a memory, or saved in a record.

## 0.831.0 - 2026-10-23
<!-- big -->

### New

- **Your teammates in Telegram.** Write to your teammates from Telegram, wherever you are, while Locust is open on your computer. Make a bot of your own with Telegram's @BotFather, paste its token in Settings > General > Telegram, and press Start in the chat its link opens: `/ada` and a message reaches Ada, and the answer comes back in the chat as a reply, with the bot typing while Ada works. After that, just write; it goes to the same teammate and carries the conversation on, and a reply to an older answer carries that one on. `/new` starts a fresh conversation, `/who` lists your teammates and `/waiting` shows the cards waiting on you. A card from a turn you started there comes to the chat with Allow once and Deny; Always stays on your computer. Only the chat that paired is answered, and anyone else who finds the bot is ignored. The token is kept encrypted on your computer. Telegram's servers can read what is written there, so Locust takes out anything shaped like a key before it sends, and Settings says so. A message that arrived while Locust was closed starts nothing; the chat says so, and you send it again.
- **A new teammate interviews you.** An empty Instructions box asked you to know how to brief a teammate before it had done anything. Now the New teammate window offers Let it interview you instead: the teammate's first conversation opens with your ask, it asks you a few short questions one at a time, and then writes its own standing instructions. They come on a card under its answer, in a box you can change, with Save; nothing is kept until you press it, and a key in them is refused. The interview's first turn runs in Ask, so it reads and changes nothing. An existing teammate's window offers the interview again.

## 0.830.0 - 2026-10-23
<!-- big -->

### New

- **Locust in a browser.** The whole window, in any browser, driven by your computer: open app.locust.lol/desk on a laptop, a tablet or a work computer, and it is the same Locust, your conversations, teammates and Board, sending messages that run on your computer and streaming their answers back. Pair the browser as you pair a phone (Settings > General > Your phone; a computer opens the code's link instead of scanning it, with Copy the link), then turn on Full window for it there. The browser loads your version's own window from Locust's relay, never code from your computer, and everything it sends and hears is sealed between the two, as your phone's is. While a browser has the window, your computer's window says Locust is open in a browser, with Use Locust here to take it back; one window acts at a time, so nothing runs twice. What stays on your computer stays there: signing agents in, permissions and the rules that grant them, the organization's policy, spending, the terminal, installing, and pairing more devices.

### Improved

- **A browser is named for what it is.** A paired computer's browser shows in Settings as Chrome on Windows or Safari on Mac rather than Browser, and the phone's page links to the full window.

## 0.829.0 - 2026-10-23
<!-- big -->

### New

- **Scout looks while your computer is off.** Turn on Look while my computer is off in Settings > General, under Scout, and Locust opens a pull request in your folder's GitHub repository adding a small workflow and the one file it runs; merge it and it is on. With your computer off, the workflow does Scout's GitHub look every hour (every two on a private repository, about 360 of its 2,000 free Actions minutes a month), with its own read-only token, and leaves what it found at Locust's relay, sealed with a key only your devices have. Your phone shows it under While your computer is off, buzzes for what can't wait, never in your quiet hours, and brings the morning brief. With Locust open it does nothing more, and when your computer is back the cards are on Home, marked as found while your PC was off, told already, and in What Scout saw. It needs Your phone, from anywhere on, and an owner or admin of the repository to add its secret. Off deletes the secret and turns the workflow off.
- **Scout's cards on your phone.** A Scout alert used to open a page with no Scout on it. The phone now lists Scout's cards in Locust's own words, newest first, with Open on GitHub and Dismiss. What a card quotes, Scout's model's take and a teammate's draft stay on your computer.

### Fixed

- **Scout's looks are not conversations on your phone.** The phone listed a look of Scout's, which began "You are Scout", among your conversations; the window never has. It no longer does.

## 0.828.0 - 2026-10-23
<!-- big -->

### New

- **Scout prepares the fix before you look.** Turn on Prepare fixes before I look in Settings > General, under Scout, pick the teammate who prepares them, and when the folder's check starts failing that teammate fixes it in a copy of your folder while Scout's card says so. Locust runs the check there again, and the card then reads "Fix ready: 2 files changed, the check passes", with Review, Apply and Skip; or, when it couldn't, what was missing, in the teammate's words. Apply brings the change into your folder beside your own edits, and says so when you changed the same files since, with Prepare again. Nothing reaches your folder before Apply, and from the copy nothing can push, add a remote or use gh; opening a pull request stays your step. It runs on that teammate's plan, so it is off until you turn it on, at most two a day unless you change it, two tries and 30 minutes a fix, and Scout's today line counts them.
- **What Scout saw.** Scout writes down every look for 14 days: what it raised, what it set aside and why (told you already, or past the three new issues a look raises), the watches it didn't read, when GitHub couldn't be read, and looks not taken in quiet hours. Settings > General shows it under Scout, newest first, with a filter for what it left out, and a card that was kept off your phone says why. Ask Scout in its own conversation "why didn't you tell me about #42?" and it answers from that record, naming the look, with no tools.

### Fixed

- **A fourth new issue is raised.** Scout raises three new issues a look, and they were always the three newest, told or not, so a fourth stayed untold for as long as three newer ones were open. It now raises the newest it hasn't told you about.

## 0.827.0 - 2026-10-23

### New

- **Alerts on your phone.** Paired from anywhere, your phone can buzz when something on your computer needs you while you're away from it: a run waiting on your approval or answer, a run that hit its plan's limit, Scout's urgent cards, and a card that could wait, left untouched for four hours. Turn them on from the phone's page; on an iPhone, add Locust to your Home Screen first. Your phone stays quiet while Locust's window is in front or the page is open, buzzes at most once every two minutes (anything else joins the next buzz), holds anything but approvals past five a day, and keeps Scout's quiet hours, unless you let approvals through at night in Settings. A lock screen says who and what kind, never a command or a message, and each alert is locked on your computer for that phone's browser alone.
- **The morning brief says what it checked.** A quiet brief no longer leaves you wondering whether Scout looked: it names what Scout watched and found quiet, says when GitHub couldn't be read instead of calling it quiet, counts what Scout set aside for having told you already, and lists the alerts it held back from your phone in quiet hours or past the day's limit.

### Fixed

- **A turn on a runtime's default names its model while it runs.** A turn Locust carried on with Claude Code read just "Claude  High" until it finished, and for good when no turn on Claude's default had finished before. The model is named as the turn starts.

## 0.826.0 - 2026-10-23
<!-- big -->

### New

- **Your phone reaches Locust from anywhere.** Turn on Your phone in Settings > General, scan the code once, and your phone shows your conversations, answers approval cards and sends the next message from any network, not only your home Wi-Fi. It goes through Locust's own relay, and every message is locked on your computer and opened on your phone, so the relay passes along what it cannot read. Settings lists each paired phone and whether it is open now, with a Remove that stops it at once. A code pairs one phone, once, within ten minutes. Only on this Wi-Fi still works as it did.

## 0.825.0 - 2026-10-22

### Fixed

- **Ask and Plan on Cursor never borrow a teammate's connector permission.** While a Cursor teammate works in Edit, Locust lets it use your connectors by writing that permission into the folder, and Cursor reads it from the folder for every turn there. A Cursor turn in Ask or Plan in the same folder would have had it too: measured, with such a permission an Ask turn used a server's tool and its file was written. Such a turn is now not started; it says why, and to send it again when the other turn ends.
- **An old Codex is told to update.** Ask and Plan on Codex read your Codex settings first (0.824). A Codex too old to answer that refused every Ask and Plan turn with "Send it again", which could never work; it now says to update Codex.
- **"Open the comment" is a button again.** On the card for a posted `/locust` answer it stretched across the card like a text box.

## 0.824.0 - 2026-10-22

### Fixed

- **Ask and Plan on Codex start none of the programs your Codex runs as MCP servers.** Such a server runs outside Codex's read-only sandbox, and Codex ran its tools without asking whenever the server said they only read: measured, a tool that wrote a file while saying so wrote it in Ask. Locust now switches off, for that turn only, every MCP server your Codex runs as a program and every plugin's, and Codex's own browser and computer use, which act on things outside the folder. Your Codex settings are not changed. Connectors reached at an address stay, and Codex still runs only their tools that say they read. If Locust cannot read your Codex settings, the turn does not start, and says why.
- **Ask and Plan on Copilot leave your own MCP servers off.** Copilot's Ask allowed every tool it was not told to refuse, an MCP server's among them. Locust now tells Copilot to leave off every server in your Copilot settings in these modes.
- **Scout reads only what it is given.** Scout's look reads other people's words, like pull request titles, check output and comments, and it ran as an ordinary Ask turn, which on OpenCode could read files and fetch web pages. It now runs sealed: no tools on OpenCode or Codex, and no memory brief, browser or skills on any model.
- **Scout counts a look on the day it began.** A look that started just before midnight and asked its model just after it counted that turn on the new day, beside no look at all.
- **Scout's notes say how many it keeps.** Settings now says it keeps the newest 40 lines, each up to 240 characters. Longer notes were cut without a word.

## 0.823.0 - 2026-10-22

### New

- **Ask a teammate from a GitHub comment with /locust.** Write a line starting `/locust` in a comment of yours on an issue or pull request in the folder's GitHub repository, and Scout's next look brings it to Home as a card that needs you, with the step drafted: your ask, its link, and the thread's text marked as other people's words. `/locust Ada ...` sends it to Ada; otherwise Scout suggests a teammate. Send starts the turn, and when it ends its answer comes back on another card with "Post it on #212". Nothing is posted until you press that, and only the text the card shows, which you can change first. Only your own comments count, each once, and only ones written after the watch was turned on. It is on by default under Settings > General > Scout keeps watch, as "Your /locust comments", and reads only on Scout's own looks.

### Fixed

- **Ask and Plan on Cursor start no MCP server a folder brings.** Locust told Cursor to approve every MCP server in every mode, so in Ask and Plan a server named in a folder's own `.cursor/mcp.json` was started without anyone being asked: measured, one told to use no tool still ran that server's command. In these modes Locust no longer approves them, and such a server does not start. The servers you set up in Cursor yourself still load, and Cursor's own Ask mode still refuses their tools unless your Cursor settings allow them.
- **A plan or quota used up says so once.** When Muse Code's plan or Antigravity's quota ran out, the conversation said it three times over, beside lines like "Unhandled Muse task state". Now a "Usage limit reached" card says when it resets, in your local time, and the turn's error is one sentence that says what to do. Muse Code 1.4.11's newer records no longer show as "Unhandled" lines.
- **Scout never wipes what it kept.** If Scout's file could not be read when Locust started, Scout began empty and its next save wrote over the file: its settings, cards and notes gone. A file it cannot read is now left alone, and one that is damaged is kept beside the new one.
- **Turning Scout off, or its quiet hours, stops a look already under way.** A look that was waiting on the folder's check could still raise cards, notify Windows and ask its model after Scout was turned off or quiet hours began. It now stops, and keeps nothing. A check result from while a teammate's turn was starting is dropped and run again later, so half-done work no longer reads as a failure needing you.
- **The morning brief says what is true.** It names the span it counts ("Since Friday" after a weekend), says "at least" when there were more turns than it read, leaves out pull requests it could not read just then, says when the folder has changed since the check last ran, and counts a check that turned as one card.

## 0.822.0 - 2026-10-22

### Fixed

- **A refusal in Ask or Plan on OpenCode says it was the mode.** When Ask or Plan refused something on OpenCode, the model was told you had declined it, and its reply said so ("you declined the permission request") though you had seen no card. It is now told that Locust refused it without asking anyone, because the teammate is in Ask or Plan mode, and the turn's line says "refused" rather than "declined".
- **A card for one of your own MCP tools shows no empty detail.** In Approve each, the card for a tool from your own MCP servers named the tool and then showed "Exact *", because OpenCode gives no details for it. The line is left out now.

## 0.821.0 - 2026-10-22

### Fixed

- **Ask and Plan on OpenCode run none of your own MCP tools.** OpenCode allows any tool it is not told about, so a tool from an MCP server in your own OpenCode settings ran in Ask and Plan without asking: measured, one that saves a note wrote its file into the folder. In these modes OpenCode now refuses every tool except the ones that read (files, searches, a to-do list, a subagent that keeps the same rules, a skill), the web's reads, and Locust's own git tool and browser, which asks before a login, payment or send. In Approve each, your own MCP tools ask on a card like everything else that acts. A `.env` file is not read in Ask or Plan, and asks first in Approve each.

## 0.820.0 - 2026-10-22

### Improved

- **A Locust left open keeps up with new versions.** It looks for a new version every hour instead of every six, and it keeps looking while a downloaded version waits: a newer one is downloaded in its place, so Restart and install brings you the newest. It used to stop looking once a version was downloaded, and offered that one until you restarted, however many came out after it. A look that finds nothing new leaves the Restart and install line where it is.

### Fixed

- **Routine inputs read whole.** In the routine editor, an input's Required checkbox stood alone in the middle of a row, above its word; it sits beside it now. And a box for more than one line, such as a choice input's list or a long text input, showed a line and a half with its words against the top edge. Each shows its lines now, as does the question box in Tune this teammate.

## 0.819.0 - 2026-10-22

### Improved

- **A teammate on a free model OpenCode dropped is told before you send.** OpenCode retires free models on its own schedule, and a teammate saved on one used to fail at its next message with "Model not found". Now its model reads amber on Home and on the Team screen, and its conversation says "OpenCode no longer offers ..." over the chat box, with one button that moves the teammate to the steadiest free model OpenCode lists. Nothing is sent until you send.

## 0.818.0 - 2026-10-22

### Fixed

- **Ask and Plan on OpenCode write nothing, a shell redirect included.** Since 0.744 these modes let OpenCode's shell run git commands that only read, and OpenCode checked such a command without its redirect: `git status > notes.txt` wrote the file, even outside the folder. The shell now refuses every command in these modes again, and git reaches the teammate through Locust's own git tool instead: status, diff, log, show, blame and branches, run with no shell and nothing written. `/review` in Ask mode reads the change through it.
- **A tool call OpenCode could not run says so.** A turn's activity line read "Used invalid" when a model reached for a tool it does not have; it now says it made a tool call that did not work. Locust's git tool reads as the git it ran, such as "ran git status, diff and log".
- **A routine stopped by a refused step says why Continue is off.** When a step was refused before it started, such as one too long to send, the routine's review card also said the step before it was never confirmed finished, though it had finished. It now says Continue would be refused the same way.

### Improved

- **Scout's morning brief and good news are Locust's alone.** Scout's model is no longer asked about the morning brief or a check that passes again, which it answered with filler and a draft to run the passing check once more. A look with nothing else new asks no model at all.
- **Free models OpenCode dropped are gone from Locust's order.** Space Bunny and LongCat left OpenCode's free list, so Scout no longer falls back to them.

## 0.817.0 - 2026-10-22

### New

- **Scout brings a weekly tune-up.** A week after Scout starts counting, its first look brings one card on Home: how many looks it made and how many needed its model, the cards it raised, and what you did with them: sent on, changed first, dismissed or left. When the week shows something worth changing, the card offers it as one button, such as Stop watching GitHub after a week in which you dismissed every GitHub card and sent none on, or Look every 30 minutes after a week that needed nothing. Nothing changes until you press it.

## 0.816.0 - 2026-10-22

### New

- **Scout brings a morning brief.** Pick an hour from 6 to 10 AM under Morning brief in Settings > General, and the first look from that hour on leaves one card on Home: who is waiting on you, what finished or failed since yesterday, and how this folder's check stands. Once a day, and never in quiet hours.
- **Scout keeps notes you can read.** When a look learns something worth keeping, such as who owns the Mac build, Scout adds a dated line to its notes and reads them on every look. They are plain lines in Settings > General: change or clear them there, and the next look reads what you kept.
- **An issue link shows its state.** A GitHub issue link in a conversation now has a chip beside it, as a pull request link does: Open in green, Closed in purple, Not planned in grey, with the issue's title on hover. It is read through the GitHub CLI you signed in, only while the link is on screen.
- **Everything waiting on you is on the Board.** A Scout card that needs you now, and a routine waiting for you to keep or discard its work, now count in the title bar's "needs you" and sit in the Board's Needs you column, each with its own buttons.

### Improved

- **Your own OpenCode skill paths keep working.** An OpenCode teammate that Locust gave skills to lost the `skills.paths` in your own OpenCode settings for that turn. It now keeps the ones OpenCode itself would use, beside its own. If your OpenCode settings file can't be read, the message names the file.
- **OpenCode can read a skill's own files.** In Ask and Edit, an OpenCode teammate could read a skill but not the files beside it, such as its references. It can now, in that turn's copy of the skill only.
- **/setup counts the skills your agents really find.** Linked and nested skill folders now count, and so do the project's skill folders up to its repository. The line reads "19 skills in your skill folders, read by OpenCode itself".
- **Picking a folder like .claude asks in the window.** Choosing one in the folder picker no longer brings up a Windows dialog. Your first message there asks in the window instead, as a new conversation in such a folder already did.

### Fixed

- **A skill that does not copy whole is left out whole.** A skill that could only be partly copied for a turn reached it half there. Now a turn gets all of it or none of it, and the rest still load.
- **Two skills with one name reach a turn once.** Names are compared as the disk compares them, so on Windows "Review" and "review" are one skill, and the one /tune is trying out wins.
- **A turn that fails to start leaves no copy of its skills behind.**

## 0.815.0 - 2026-10-22

### New

- **Tell Scout what to keep watch for.** Settings > General has a box for it, in your own words, such as "keep main's tests passing, and tell me about new issues on the sign-in page". Scout reads it on every look and says how what it finds bears on it.
- **Scout can watch this folder's check.** Turn on This folder's check and Scout runs the check from Settings > Project folder whenever the folder has changed since its last look, never while a turn is working there. A check that starts failing is a card that needs you now; when it passes again, that card replaces the failing one.
- **Quiet hours for Scout.** Pick hours such as 10 PM to 7 AM: Scout does not look on its own then and sends nothing to Windows. The first look afterwards catches up on everything.

### Improved

- **Scout waits while you're working.** A look that falls due while a turn you sent is running waits for it to end, at most half an hour. Turn it off in Settings if you would rather it looked anyway.
- **Scout says what today used.** Its line in Settings counts today's looks and how many of them needed its model.

### Fixed

- **Turning Scout off stops a look under way.** The timer stopped, but a look that had already asked its model kept going. Now it is stopped, and its cards stay in Locust's words.

## 0.814.0 - 2026-10-22

### New

- **A message that names its project starts there.** Paste a handoff such as "Read C:\work\shop-wt\fix-login\docs\BRIEF.md and do what it says" into a new conversation while Locust is in a folder like .claude, or in another copy of the same repository, and the conversation starts in the project that file is in, as if you had switched folders first. A line under it says where it went. A message that only mentions some other repository is read where you are, as before.

### Improved

- **A routine's fix is said once.** When a routine keeps going until its check passes, the fix turn's own line says the check failed and was sent back. The note under it that said so again is gone, and the check's result now comes before the routine's "finished" line.

## 0.813.0 - 2026-10-22

### New

- **Skills reach Codex, OpenCode, Copilot and Antigravity teammates.** A teammate's own skills, the ones a tune drafts and the ones you keep from GitHub, now reach those four as well as Claude Code. Each run is handed its own copy, and nothing of yours is changed: each still finds the folder's skills and your own itself, as in your own sessions. /tune now drafts "What it knows" as a skill for them too, and /setup lists the folders each one reads.

### Improved

- **/setup before the first turn.** /setup now works in a teammate's conversation before you have asked it anything, so you can see what it will be given before its first turn.
- **A routine's fix reads as Locust's.** When a routine keeps going until its check passes, its fix turn now reads as Locust's line, such as "node check.js failed, so Locust sent it back to Cedar · fix 1 of 2", with What Locust sent one click away. It used to read as a message from you.

### Fixed

- **A conversation's check is its own folder's.** Locust switches folders without restarting, and a conversation keeps working in the folder it began in. Its check after edits, and what a failure may do, came from the folder on screen instead, so switching while a turn was working could run another folder's check, or send a failure back on another folder's setting. Both now come from the conversation's own folder.
- **A fix never runs beside your reply.** If you replied just as Locust was sending a failing check back, both could start at once in the same conversation and the same files. Locust now leaves the conversation to your reply and says so.
- **A first message says OpenCode is setting itself up.** The first time OpenCode starts on a machine, it installs a package of its own, about a minute here, before it answers. The first message's line said "Looking for OpenCode" for all of that; it now says OpenCode is setting itself up for its first start.

## 0.812.0 - 2026-10-22

### New

- **Fix until the check passes.** Settings > Project folder now has a choice under Check after edits. When it fails, Show me keeps the card and its Send button, as before. Send it back to fix, at most 1 to 5 times, hands the failure back for you. After a turn you sent, when your check fails with something new, the same teammate gets what the check printed, on the same model, and the check runs again after its fix. It stops when the check passes, when the fixes run out, when a fix changes nothing, or when all that still fails was failing before your turn, and it says why under the turn. In the conversation a fix is Locust's line, such as "npm test failed, so Locust sent it back to Wren · fix 1 of 2", never a message from you, and What Locust sent is one click away. It is off until you choose it, since each fix is one more turn. The idea comes from agent tools such as OpenCode and Aider, which hand errors straight back to the model.

### Improved

- **A check that still fails reads plainly.** When nothing in a failure is new, its card now says it "still fails after this turn, with nothing new in it". It used to say the check failed "the same way it did before", which was wrong when part of the failure had been fixed.

## 0.811.0 - 2026-10-22

### New

- **Your benchmarks, read across.** The Benchmarks screen now opens with one table for the models you ran in two or more benchmarks: each one's average score, with every benchmark counted once, beside its runs that met every point, its time and its cost. One table instead of one per question.
- **A tune can be measured before you keep one.** One run of each setup can be luck. After a tune's three setups answer, ask a judge to score them, then run them again: the benchmark reads a row a setup, still blind as Setup A, B and C, so you keep the setup whose scores hold up. Note what this tune showed then reads every run, not one.
- **Combine both edits.** After a tune, when you kept one of the two drafts, Combine both edits asks the teammate to write that draft's edit and the other draft's as one set of instructions. It says what it took from each, and you read it, then use it or not; nothing changes until you press Use. The idea comes from Google Research's Harness².
- **A tune can give a Claude Code teammate a skill.** When you /tune a Claude Code teammate, "What it knows" is drafted as a skill: a name, when to use it, and what it says, kept beside the teammate's own instructions. A skill loads only when the work matches it, so it costs nothing on other turns. You can edit it on the card, switch back to instructions, or write one yourself, and only that setup's run is given it. If you keep that setup, press Use this skill for the teammate; its later turns get the skill. Edit teammate lists its skills, each with Remove.
- **See what shapes a teammate.** Type /setup in a teammate's conversation to see everything its turns are given, in order: its folder, LOCUST.md, the runtime's own file such as CLAUDE.md, its group's and its own instructions, your About you note, and team memory. Then come the skills and connectors it loads when the work matches, the lessons only /tune reads, and what its last turn loaded, as Claude Code reported it. It changes nothing, and Edit teammate is one click away.

### Improved

- **A check that is running says so.** After a turn that changes files, the card under it reads "Running the check npm test after this turn…" until your check finishes. In a comparison, Ask a judge says that a judge asked meanwhile waits for it.
- **Compared models read by name.** A built-in model in a comparison, its benchmark and the benchmark's chart reads as the picker names it, such as Deepseek V4.1 Flash Free, not opencode/deepseek-v4.1-flash-free.

### Fixed

- **Chart labels no longer print over each other.** Two models at the same score, one label running right and one running left, now sit a line apart, each joined to its point.

## 0.810.0 - 2026-10-21

### New

- **A comparison's judges read the work.** When the columns changed files, each judge now sees what each one changed, beside its answer and still without names. A claim about the changes is checked against them, and a column that changed nothing is said to have changed nothing. The idea comes from Google Research's Harness².
- **Judges see your check.** In a comparison that changes files, the check you set for the folder (Settings > Project folder) runs in each copy after its turn. Its result now shows under each answer and beside the judges' score, and every judge reads it before scoring: a failed check counts against an answer, whatever the answer says it did. What the check printed reaches the judge with your folders' paths left out. When the check fails the same way in every copy, for example because it needs dependencies a copy leaves out, Locust says so, and the judges are told it tells the answers apart on nothing.
- **Your own models can judge.** A model you added with your own key is offered as a judge, under Your models. A model that only chats is not, since a long comparison reaches a judge as files.
- **Your own models are priced.** A model you added at its provider's own address, such as Anthropic's or OpenAI's, under the name that provider lists, is now priced from the model list OpenCode keeps, including the higher rate some models charge for a long context. Its runs, your benchmark and the benchmark's chart show dollars instead of tokens, and its runs count toward a teammate's monthly spend limit. A model on a gateway or a server of your own is still counted in tokens.
- **A tune can leave a lesson.** After you keep a setup, Note what this tune showed asks which edit made the difference, judged from what the runs did, and may write one lesson, such as "When a teammate gives a short answer, add a line capping its length." Lessons are read only when you tune that teammate again, never in its everyday work, and Edit teammate lists them under Tune lessons, where you can remove one. Often there is no lesson, and that is normal.

### Improved

- **Your benchmark reads more plainly.** Running it again says "Running it once more" or "Running it again, 2 of 3" instead of "Run 1 of 1" beside the benchmark's own count, three judges are called judges, a chart label moved to clear another is joined to its point by a line, and a Check passed column counts the runs whose copy passed your check.
- **A model added under its own id reads by its name.** A model you added with its id as its name, such as claude-haiku-5-5, reads as Claude Haiku 5.5 in Compare, your benchmark and the judge picker.
- **OpenCode 2 steps say what they did.** A step is named from the tool it ran, and a folder is never shown as if it were the command.
- **A teammate's brief is shorter.** With nothing remembered yet, it no longer explains how to read memories, about 1,100 characters less on every new project's first turn, and the line for suggesting an About-you sentence is taught once instead of twice.

### Fixed

- **OpenCode teammates starting at the same moment no longer lose one.** When several started together, one could stop with "database is locked" before it began. It now starts again a moment later.

## 0.809.0 - 2026-10-21

### New

- **OpenCode 2 works as a teammate.** OpenCode 2 changed how other apps talk to it, and Locust now speaks its new way, so an OpenCode teammate works with OpenCode 2 installed instead of reading as not supported. Its models fill the picker straight from OpenCode, free ones included, and Ask, Edit, Auto and Approve each all work: Approve each asks you on a card, and a request you decline reads as declined. Locust's command guard stops the same risky commands it stops for the other agents. OpenCode 1 keeps working as before. Built by Mit Godhani.
- **Tune a teammate.** Type /tune in a teammate's conversation after a turn. The teammate drafts two edits to its standing instructions, one about how it works and one about what it knows, each drawn from a moment in the conversation. Locust then runs your last ask three ways at once, blind, as Setup A, B and C: the current instructions and the two drafts. Keep the one that did best, then choose whether the teammate uses those instructions from now on; nothing changes until you press Use these instructions. The card says when a draft is the same as the current instructions. The idea comes from Google Research's Harness².

### Improved

- **A judge treats each answer as material to judge.** An answer that tells the judge which one to keep, or how to score it, is read as part of that answer and never followed.

## 0.808.0 - 2026-10-20

### Improved

- **Home fits on one screen while Scout is on offer.** Meet Scout was a large card above your team, and Home scrolled at common window sizes until you decided. Now Scout is offered in its own place on your team, the seat it takes once it is on. Press it to read what Scout reads and with which model, then turn it on. Its x means Not now.

## 0.807.0 - 2026-10-19

### Improved

- **The Team board says what a teammate is waiting on.** A teammate who needs you shows the question they asked, worded as Home words it, with Answer to open that conversation. Their last run reads whole ("15 minutes ago" was cut short), and Message, Edit and Remove show faintly before you hover. The sidebar's Team button lights up while the board is open.
- **The model picker is shorter to read.** A model no longer repeats its group's name under it, and READY is gone, since the dot already says it. Only tags with news stay, such as SIGN IN or QUOTA EXCEEDED, so more models fit before scrolling.
- **Steps line up.** In an opened list of steps, commands start on the same edge as file names, with their result after them.
- **A decision card looks like it is waiting, and nothing looks chosen.** The card sits raised, each option's number is drawn as a key you can press, and option 1 no longer wears a focus ring before you choose anything.
- **The + menu says what each button does.** Open, its two buttons are labeled Attach files and Choose a folder.
- **Home folds your accounts sooner.** With a team, an agent that is still being checked no longer holds the full accounts list open; the line says "6 ready · checking OpenCode" until it answers.

### Fixed

- **Share your team, Save the record and Your cloud computer close with Escape.** They also keep the keyboard inside while open, and each has a close button like the other dialogs.

## 0.806.0 - 2026-10-19
<!-- big -->

### New

- **A long conversation has a rail of your own messages.** Once a conversation scrolls and has four or more of your messages, a slim strip at its right edge marks each one. Hover a mark to read its first line; click it, or press Ctrl+Alt+Up and Ctrl+Alt+Down, to jump back to it. It steps aside when a panel beside the conversation leaves no room for it.
- **The context ring says what fills a Claude conversation.** Pressing the ring now lists Claude Code's own estimate for the last finished turn: the conversation, tools, skills, the system prompt and the room left, with a note when something you control takes a large share. Reading it costs nothing and adds nothing to the conversation.

### Improved

- **References work with Claude in Edit.** A reference was left out of a Claude conversation in Edit, because Edit could have changed it. Now Claude reads it there too, and Claude Code's own file rules keep it from changing it.

## 0.805.0 - 2026-10-19

### Fixed

- **OpenCode 2 is said plainly instead of failing every turn.** OpenCode 2 changed how other apps talk to it, so with it installed every OpenCode turn in Locust ended with "OpenCode's event stream closed before the turn finished" while OpenCode still read as ready. Locust now shows OpenCode 2 as not supported yet, says why on hover, and how to keep using OpenCode teammates today (OpenCode 1). Support for OpenCode 2 is on its way.

## 0.804.0 - 2026-10-19
<!-- big -->

### New

- **A conversation can read other folders.** From the folder chip, Add a reference: a folder on this computer, or a GitHub repository, which Locust copies with your own gh sign-in (up to 500 MB). The teammate can read it and is told it is for reading, not changing; your mode stays exactly as you chose it. Where a mode could change the folder, or adding it would allow writes the mode does not, the reference's chip says so. References stay with the conversation, up to five.
- **A pull request link says how it stands.** A link to a GitHub pull request in a conversation now shows Open, Draft, Merged or Closed beside it, and whether its checks pass, read through your gh sign-in and kept up to date while checks run.

## 0.803.0 - 2026-10-19

### Improved

- **The folder chip stays grey in a folder like .claude.** It turned amber there on every conversation, even ones you keep there on purpose. Now it looks like any folder; its hover still says why such a folder is risky, and Locust still asks once before the first conversation there.

### Fixed

- **A Copilot or own-agent turn ends when the agent's output stops.** If an agent that speaks the Agent Client Protocol (Copilot, or one you added in Settings > AI agents) closed its output mid-turn while its process lived on, the turn could wait for a timeout. Now it ends at once and says so, keeping a reply that had already arrived in full. This is also the groundwork for OpenCode 2, which Locust will run the same way. Built by Mit Godhani.

## 0.802.0 - 2026-10-19

### Improved

- **A shared team brings what it knows and does.** Share team's image now carries each teammate's instructions and the first messages they offer, and their routines in this folder, as well as their names, looks and models. Routines arrive paused: on no schedule, read-only, and running only when started; a path on your computer becomes something the other person fills in. After adding, Locust says who came with instructions so you can read them first.
- **Build a landing page is for anything.** The Home row no longer builds a coffee shop's page: it asks for a page with a headline, what it offers and a way to get in touch, and leaves the box open for you to say what it is for.

### Fixed

- **Add team from image follows your organization's policy.** A teammate whose model the policy does not allow comes without that model, and picks one it may use.

## 0.801.0 - 2026-10-19

### Improved

- **Answer a question by number.** When a teammate stops to ask which way to go, each option wears a number: press it and that answer is sent, as in Claude Code. The card takes the keys when the chat box is empty, and starting to type a reply instead still goes to the chat box. A question card that has several questions works the same way, then Enter sends.
- **An option's note sits under its name.** On the same cards, what each choice means now lines up under its label.

## 0.800.0 - 2026-10-19

### Improved

- **The chat box fits beside the browser.** With the browser open, the chat box's buttons ran to three rows with Send alone on the last. Now they are two, with Send at the end of the second, and the model chip shows the model's name there; its hover still says the whole route.
- **Settings › Chat reads like the rest of Settings.** Where your commands live, on this computer and in this folder, are rows with Open folder at their end.
- **An Antigravity turn that has not started says so.** If Antigravity takes a turn but writes nothing for it, usually because its window was closed or crashed, the conversation says so after a minute instead of after ten.

## 0.799.0 - 2026-10-19
<!-- big -->

### New

- **Start a conversation from a GitHub issue.** In a folder that is a GitHub repository, Start from an issue sits beside Compare models on Home. It lists the folder's open issues, and choosing one fills the chat box with the task, its link, the issue's text and its last three comments, marked as written by other people. Its pictures come along too, from GitHub's own image hosts only. A teammate working in its own copy gets a branch named after the issue, and a pull request from that conversation says which issue it fixes. Nothing is sent until you press Send.
- **Bring in OpenCode and Cursor Agent conversations.** Import a conversation now lists your OpenCode and Cursor Agent sessions from the last 30 days, beside Claude Code and Codex, each marked with its agent. Your next message continues the same session. Cursor's chats are only read, never changed.

### Improved

- **Home fits on one screen.** Your connected accounts and usage now sit under the cover, then your team, then the starters, two to a row. Scout's watch is the line on its own card, with Look now as the eye in its corner, instead of a row of its own. With six teammates, Home no longer scrolls at 1205x803, 1366x768 or 1440x900.
- **Ask for a review is one row.** In a conversation's More actions, it opens your reviewers beside it, with two teammates on different agents below them. One reviewer is still one row that names them.

## 0.798.0 - 2026-10-19
<!-- big -->

### New

- **Point at a part of a page.** In the browser beside the conversation, the pointer in its toolbar (or Ctrl+Shift+C) lets you click any part of the page. It lands in the chat box as a card with its picture, its HTML, the styles that shape it and, when the page names it, the file and line it comes from. Add a note, point at more, and Send when ready. What is typed into a form never comes along, and a password or card field is never pictured.

### Improved

- **Turns are about ten times lighter: a teammate's connectors are off until you tick one.** Claude Code sends every tool of every connector on your account with each message, about 170,000 tokens a turn on an account with ten of them. A teammate now carries your connectors only once one is ticked in its settings, under Connectors, and a message that names a connector it has off says so before you send it. Ticked ones work without asking, and the rest ask first.

## 0.797.0 - 2026-10-19
<!-- big -->

### New

- **More than one account per agent.** In Settings > AI agents, Claude Code and Codex have Add another account: give it a name, such as Work, and sign in with the agent's own sign-in in your browser. Pick it on a teammate or in the model menu, where it reads "Claude · Work", and that teammate's turns, comparisons, routines and handoffs use that sign-in, with its usage counted on its own. Your PC's sign-in stays as it is and is used when no account is picked. Remove deletes only the folder Locust made for that account. A company's policy file can keep added accounts off.
- **Run a command from the chat box.** Start a message with ! and it runs in the conversation's folder, in your terminal's shell. The command and its output appear in the conversation and go along with your next message to the teammate, once. Stop ends the command and anything it started.
- **Your own slash commands.** A Markdown file in Locust's commands folder, or in a project's `.locust/commands`, becomes a slash command in the / menu. What you type after its name fills `$ARGUMENTS`, `$1` and `$2`, and a !`command` line puts that command's output in. Choosing one fills the box; Send sends it. A project's command lines run only after you allow that folder once. Settings > Chat lists your commands, with Open folder and how to write one.
- **Compact a conversation from its context ring.** With Claude Code, the small circle beside the model shows how full the conversation's context is. Press it for the numbers and Compact now, which summarizes the conversation so far so the turns ahead have room, and leaves what you were typing in the box. While a turn is running, it waits and compacts when the turn ends.

### Improved

- **The conversation's header is a row of icons, as Claude Code's is.** Activity, Terminal, Changes, Browser, the folder and the menu are icons, each named when you point at it, and the folder icon opens the folder your teammates work in. Open in terminal is in the ... menu.
- **The model chip names your default model.** A route on an agent's own default model read just "Claude", so a turn carried on with Claude Code when another agent's plan ran out said nothing about the model it ran on. Once a turn has said which model the default is, the chip names it, for example Claude / Opus 5.5.

### Fixed

- **Scout no longer asks to redo work that was carried on.** A failed turn whose conversation went on, with your reply or with Locust carrying it on when a plan ran out, leaves Scout's next look, and a card already showing it goes.

## 0.796.0 - 2026-10-19
<!-- big -->

### New

- **A browser your teammates can use, beside the conversation.** Press Browser at the top of the window, and Locust's own browser opens beside the conversation: a session of its own, apart from your Chrome and its sign-ins, with tabs and an address bar. Your teammates can open pages in it, read them, click, type, take a screenshot and read the console and the network, so a teammate that builds a page can check that it works, and a quiet line names the teammate while it acts. Before it types a password or a card number, submits a form, buys, sends, downloads, or leaves for a site you have not opened, it asks you on a card in the conversation. Teammates on Claude Code, Codex, OpenCode, Copilot and agents you added have it; Cursor, Antigravity and Muse Code cannot take it yet, and the panel says so. A company's policy file can turn it off.

### Improved

- **Every list you choose from is Locust's own.** The fourteen that opened your system's plain list, in Compare, a new teammate, routines, Scout and the voice settings, now open the same menu as the chat box, in both themes, and work the same way from the keyboard.

## 0.795.0 - 2026-10-19

### New

- **Home's starters say what they start, and each one can compare.** Four rows replace the "Try two models on" chips: Plan before building, Debug an issue, Review my changes, and Build a landing page. Pressing a row puts its words in the box for your teammate, in the mode it suits: Plan for a plan, and Ask for a review, so nothing changes. Its Compare button puts the same words in a comparison of two models: of the work, each in its own copy, for a fix or a page, or of the answers for a plan or a review. Nothing is sent until you press Send.
- **Zoom into an image.** An image opened beside the conversation zooms with a trackpad pinch or Ctrl and the mouse wheel, around the pointer, and pans by dragging or scrolling once it is larger than the pane. Double-click or Fit puts it back, and 100% shows its own pixels.
- **Move between conversations with the keyboard.** Alt+Down and Alt+Up open the next and the previous conversation in the sidebar; with Shift, the next or previous one waiting for you. Whatever you were typing stays with its conversation.

### Improved

- **The terminal lists WSL by distribution.** On Windows the shell menu offers each Linux you have installed, by name, such as Ubuntu (WSL), and no WSL entry when there is none.
- **A new person starts on DeepSeek V4.1 Flash Free.** The free models are read again from OpenCode's list: three that are gone are dropped and five newer ones are added. A new person's first free model is now DeepSeek V4.1 Flash Free, with Space Bunny Free, which OpenCode offers for a limited time, after it.

## 0.794.0 - 2026-10-19
<!-- big -->

### New

- **A terminal beside the conversation.** Press Terminal at the top of the window, or Ctrl+`, and a terminal opens beside the conversation in its folder, or in the teammate's own copy when it works in one, before the first message too. It is yours, not a teammate's: nothing typed or printed in it goes to a model. On Windows it opens PowerShell, and the arrow beside + offers Command Prompt, Git Bash and WSL when your PC has them; on a Mac or Linux it opens your login shell. Keep up to eight tabs. Hiding the panel keeps them running, closing a tab ends its shell, and quitting Locust ends them all. A company's policy file can turn it off.

## 0.793.0 - 2026-10-19

### Fixed

- **An OpenCode turn that loses its connection keeps every step, not just its answer.** When OpenCode's live connection closed before a turn finished, Locust kept only the final answer: the commands the teammate ran, the files it read and what came back were missing from the conversation. Locust now reads the whole turn back from OpenCode and shows each step once, in the order it happened, before the answer. Found and fixed by Mit Godhani.

## 0.792.0 - 2026-10-18

### Improved

- **A company's policy reads in plain words.** Settings > General > Managed by your organization names the AI agents the way Locust does, the highest mode the way the mode menu does, and the spend limit in dollars a month, instead of repeating the policy file's code. Where the policy turns other apps off, Settings no longer offers Add to the Claude app, and says what the policy set once, beside the switch.

## 0.791.0 - 2026-10-18

### New

- **A company can set Locust's limits once, for everyone on a computer.** An administrator puts one policy file on the machine, and Locust follows it: which AI agents and models teammates may use, the most a turn may do (Auto can be ruled out), the highest monthly spend limit, and whether other apps, your phone and cloud computers can connect. Locust checks it before anything starts, and Settings shows what it sets as "Set by your organization", so it cannot be loosened there. With no policy file, nothing changes. The guide for administrators is docs/ADMIN.md on GitHub.

## 0.790.0 - 2026-10-18

### New

- **Locust asks before a conversation starts in a folder like .claude.** The folder button in the message box turns amber in your home folder, in a folder where AI agents keep their own settings (.claude, .codex, .cursor, .gemini, .copilot, .config) and in Windows' own folder, and says why when you point at it. The first new conversation there waits in the box while a card above it asks: Choose another folder, or Start here, which sends your message. Locust asks once for each folder, and choosing it in the folder picker after its warning counts as your answer.
- **Every build is on GitHub too.** The Releases page at github.com/StopBanning/locust has each build's notes, and the newest carries the same installers as locust.lol with the SHA-256 sum of each.

### Improved

- **The AppImage is tested updating itself.** Before each release it downloads an update on a stock Ubuntu, replaces itself on disk and starts again as the new version.

## 0.789.0 - 2026-10-18
<!-- big -->

### New

- **Locust runs on Linux.** Download it at locust.lol as a .deb for Ubuntu and Debian, an .rpm for Fedora and openSUSE, or an AppImage for any other Linux. Each is installed and run on Ubuntu before it is published: it opens, a teammate answers on a free model, and Stop ends everything the turn started. The AppImage updates itself. A .deb or .rpm belongs to your package manager, so it tells you when a new version is out and hands you the new package to open. Signing in to an AI agent and Open in terminal use the terminal your desktop has.
- **Light mode.** Settings > Appearance > Theme: Dark, Light, or Match computer, which follows your computer and changes when it does. Dark stays the default, so nothing changes unless you choose it.

## 0.788.0 - 2026-10-18

### Fixed

- **An OpenCode follow-up keeps its answer when the connection drops.** Since 0.786 Locust could keep the answer of a turn whose OpenCode connection closed early only in a new conversation, and a follow-up in an ongoing one ended asking you to send it again. Locust now matches the answer to the message you sent, so a follow-up keeps its answer too. While OpenCode is still working, Locust now waits for it for up to five minutes instead of giving up after ten seconds. Found and fixed by Mit Godhani.

## 0.787.0 - 2026-10-18

### Fixed

- **A file a teammate writes outside your folder opens.** When you asked a teammate to save a file somewhere else and it handed the file over by its full path, the conversation said "Not shown as a file", and the step that wrote it would not open it either. Both now open the file, show it in its folder and save a copy, just as they do for a file in the conversation's folder. Locust still opens a file outside your folders only when the teammate itself wrote it, read it or handed it over.

## 0.786.0 - 2026-10-18

### Fixed

- **An OpenCode turn no longer hangs when its connection drops.** If OpenCode's live connection closed before a turn finished, the turn could sit on Working forever. Locust now asks OpenCode how the turn went: it keeps the answer when it can confirm the turn finished, and otherwise ends the turn with a message to send it again. Stop also always ends an OpenCode turn now, even when OpenCode never answers it. Found and fixed by Mit Godhani.

## 0.785.0 - 2026-10-17

### Fixed

- **No false memory warning after an Undo.** Told its last turn was undone, a teammate could try to forget the file it had made, and on a new computer the thread said in amber that it "could not" and "It is still remembered", though nothing had ever been remembered. A forget with nothing to forget now says nothing.

## 0.784.0 - 2026-10-17

### Fixed

- **A teammate knows when you undid its last turn.** Undo puts your folder back without the teammate seeing it, so when you asked about the work afterwards it could answer from memory that the file it made "is still on disk". The next message after an Undo now tells it which files were put back and which were left as they are, so it works from the folder as it is.
- **An app connected to Locust gets the new reply, not the last one.** When another AI app (connected in Settings > General) sent a teammate a follow-up and asked for the reply straight away, Locust could hand back the previous answer while the new turn was still starting. It now says the next turn is starting until it has. Found and fixed by Mit Godhani.
- **OpenCode's first start says how long it takes.** On a new computer its line now says "about a minute and a half", which is what that setup took on clean machines, instead of "about a minute".

## 0.783.0 - 2026-10-16

### Improved

- **Updates come from Locust's own website now.** Locust used to download its updates from a GitHub account. On October 16 GitHub suspended that account without a reason, and every copy's update check stopped working ("The update check could not complete"). From this version on, updates and downloads come from dl.locust.lol, Locust's own address, so no single account can cut a copy off again. Copies older than this one still look at the old account, so they need this version installed by hand once, from locust.lol.

## 0.782.0 - 2026-10-16

### Fixed

- **Changes shows only in a folder that uses Git.** The Changes panel lists what a folder's Git branch changed, so in a folder without Git it could only say "This folder is not a git repository". The button now appears only where there is a branch to show, as Commit already did. Each reply still lists the files it changed, and Undo still takes a turn back, with or without Git.

## 0.781.0 - 2026-10-16

### Fixed

- **Antigravity's usage shows again, with Gemini 4 Argon's own limit.** Antigravity now reports a weekly limit for Gemini 4 Argon beside its Gemini and its Claude and GPT windows, and Locust threw the whole reading away when it met a group it did not know, so Antigravity's usage disappeared. Locust now reads any group Antigravity names, and Argon's weekly limit shows whenever it is lower than Gemini's.
- **Locust's links follow its GitHub account's new name, locust-lol.** Updates, Report a problem and the page listing what Locust connects to now use github.com/locust-lol. Windows copies already installed keep updating through GitHub's redirect. A Mac copy on 0.780 or earlier will not find this update by itself: download it once from the releases page, and it updates itself from then on.

## 0.780.0 - 2026-10-16

### Improved

- **A new computer's first message no longer waits on OpenCode's setup.** The first time OpenCode starts, it installs a package of its own, and the first message waited for it: a minute and a half on a clean computer, then an answer in seven seconds. Locust now has OpenCode do that setup in the background as soon as it is installed. If a message starts before it is done, its line says "Setting up OpenCode for its first start, about a minute" instead of "Starting OpenCode".
- **Undo works without Git.** Undo kept its copies of your folder with Git, so on a computer without Git no turn could be undone, and nothing said why. When Git is not there, Locust now keeps the same copies itself. Computers with Git work as before.
- **On a new computer, Home starts with the Install button.** With no AI agent installed, the free one's Install button sat under the team templates and the folder card, out of sight in Locust's own window. Until an agent is installed, it now comes first.
- **Settings can install an agent.** The AI agents page said "Press Install" but had no Install button, and the npm command it showed does not work on a computer without Node.js. It now has the same Install button as Home.

### Fixed

- **Plain words on a new computer's first screens.** The chat box, the startup screen and the send button's note said "coding agent" and "runtimes". They now say "AI agent", as the rest of Locust does. A question in a conversation with no teammate was headed "The runtime needs a decision"; it now says "A decision is needed to continue".
- **Claude Code's remote sessions switch shows only where Claude Code is.** It was the first thing on the AI agents page of a computer without Claude Code, where it could only fail.

## 0.779.0 - 2026-10-16

### Improved

- **A handoff shows the models on both sides.** When a conversation moves to another agent, the line between them now reads "Handed off", then each side's logo and the model it ran, as in GPT-6.1-Sol → Opus 5.5. The side it went to is in its brand's colour where it has one. Before, it named only the two apps.
- **Compare columns are named by the model.** A Cursor model with levels, such as Gemini 4 Argon, read "Gemini 4 Argon Low" even with its level set to High. It now reads "Gemini 4 Argon", and the level is beside it.

### Fixed

- **A Cursor teammate set to a level starts the first time.** A message sent in the first seconds after opening Locust, before Cursor's model list had arrived, could fail with "Cursor Agent takes no effort level". The level is now turned into Cursor's own model name when the run starts.
- **A finished Antigravity run is shown as finished.** Antigravity sometimes reports "The stream was interrupted" after its whole answer is in. Locust showed that run as failed, with Try again, over finished work. It now shows the answer as done, with a line saying what Antigravity reported. A run cut off while it was still working is still shown as stopped.
- **The Changes panel updates after you commit.** It used to keep showing the changes you had just committed until the next turn.

## 0.778.0 - 2026-10-15

### Improved

- **A long history no longer slows the window.** The conversation list now draws what you can see and adds more as you scroll. Each teammate's face appears as its row comes into view. With 700 conversations, a reply now streams at a full 60 frames a second, where it managed about 12, and clearing a search takes a tenth of a second instead of nearly two. Every conversation is still in the list, and the end of the list says how many are left to draw.
- **Your phone's page reads your history only when it changes.** It used to read all of it every three seconds while the page was open.
- **A very long turn counts "at least".** When some of a turn's middle is kept in its record but not shown, its summary now says "at least 596 tool calls". It no longer gives a count that reads as the whole turn.

### Fixed

- **A long answer sent in one piece no longer stops the run.** An agent that sent more than about 16,000 characters at once was stopped with "the mission ledger could not be written". This mostly hit agents you added yourself. Long text is now kept with its start and end, and the run carries on. The same goes for any other field that is too long, from any agent.
- **The phone page answers every request.** A malformed key could leave a request unanswered and show "Locust hit a problem". It is now refused like any wrong key.
- **A point that ends in a size is no longer a weight.** In a benchmark, "Draws a 3x3 grid" or "Works on Linux2" was read as a weighted point. A weight now needs a space before it, as in "Handles empty input x3".
- **The send button rests on computers that can't draw its metal.** Without WebGL, for example on a remote desktop, it checked 60 times a second, forever. It now shows the plain button after three seconds.

## 0.777.0 - 2026-10-15
<!-- big -->

### New

- **Add any agent that speaks the Agent Client Protocol.** Goose, Qwen Code, Kimi CLI, Mistral Vibe and many more start in this protocol with a command like `goose acp`. Add one in Settings > AI agents > Your own agents with its name and that command. Test starts it, asks its name and the models it offers, and stops it. No message is sent, so nothing is spent.
- **It works like any other agent.** It's in every teammate's model list, in a group of its own, with the models it offered, and its conversations are kept like the rest. It runs in Approve each: every action it asks to take is a card you answer. Which actions it asks about is the agent's own setting.

## 0.776.0 - 2026-10-15

### Improved

- **Notes on a change, from the Changes panel too.** Hover a changed line in the Changes panel and press its +, as you could in a conversation's own changes. Your note goes to the teammate with your next message, with its file, its line and the line itself. It works the same in Review changes when it's that teammate's conversation on screen.

## 0.775.0 - 2026-10-15
<!-- big -->

### New

- **Locust on your phone.** Turn on Settings > General > Your phone and scan the code with your phone's camera. Your phone then shows your conversations and their answers, and you can send the next message from it. Locust's window shows it as your own message, in the same conversation.
- **Answer approval cards from your phone.** When a teammate asks to change a file or run a command, the card shows on your phone too, under Waiting on you. Allow once or Deny there, and the card in Locust's window goes away.
- **Your phone and Locust talk directly, on your Wi-Fi.** Nothing goes through anyone else's server. The code holds a key: only a phone that scanned it can see or do anything. New code unpairs every phone at once, and turning the setting off closes the page. The first time, Windows may ask whether Locust can use your network: choose Allow, on private networks.

## 0.774.0 - 2026-10-15
<!-- big -->

### New

- **When a plan runs out, the work carries on.** When Codex or Claude Code stops because your plan ran out, the stopped turn's card now offers to carry the work on with another AI agent in one press. The next agent is told what was asked, what was done and how it stopped, and finishes the rest. A line in the conversation says where the work went and why.
- **Pick who carries on, or let it happen by itself.** Settings > AI agents > When a plan runs out picks the agent the card offers. With Without asking on, Locust carries on with that agent by itself, only for messages you send yourself. It never moves your work to an agent you didn't pick.

## 0.773.0 - 2026-10-15

### Added

- **Muse Code teammates can run in Auto.** Auto runs Muse's commands without Muse's own Windows sandbox, the way Codex's Auto runs without Codex's, so a Muse teammate can run tests and builds with nothing to set up first. Edit still keeps that sandbox; where it isn't set up, the conversation now suggests switching the teammate to Auto first, and `muse sandbox windows setup` second.

## 0.772.0 - 2026-10-14

### Fixed

- **Muse Code says when it can't run commands.** On a computer where Muse Code's own Windows sandbox was never set up, every command it tried failed before it started, and all you saw was a row of failures. The conversation now says so once, in a sentence, with the command that sets it up: `muse sandbox windows setup`.

## 0.771.0 - 2026-10-13

### New

- **Commit messages written from the changes.** In Commit, Write it from the changes asks the teammate that made them to write the message from the diff: a subject line, and a short body that a pull request takes as its description. It goes in the box for you to read and change; nothing is committed until you press Commit.
- **Walk me through it.** In Changes, the teammate that made a change goes through it as its author would in a review: what it does, then each file in the order to read it, why it changed and what to check. The files are shown in that order, each with its note, until you close it.

## 0.770.0 - 2026-10-13

### New

- **Diagrams in a reply.** A flowchart, a sequence of calls, a pie chart or a state diagram a teammate writes in Mermaid is now drawn as the diagram, in Locust's own type and colours, with its text a tab away. A wide one scrolls inside its frame instead of shrinking its words, and one Mermaid cannot read stays text, with the reason under it.
- **Links that open Locust with a message ready.** A link like `locust://new?prompt=...` in a README or on a page opens Locust on a new conversation with that message in the box. It is never sent: the box says it came from a link, and anything in it you could not see is taken out first.

## 0.769.0 - 2026-10-13

### New

- **Find in a conversation.** Press Ctrl+F (Cmd+F on a Mac) and type. Every match in the conversation lights up, with how many there are, and the one you are on stands out. Enter steps to the next, Shift+Enter to the one before, and Esc closes it. When earlier turns are folded away, Find offers to search them too.
- **Your cloud hours this month.** Settings > Cloud computers now shows about how many hours Locust kept your cloud computers on this month, on a bar against the 60 GitHub includes free, or 90 with Pro. Each computer shows its own share. Exact hours on GitHub opens GitHub's own count, which also sees your other codespaces.

## 0.768.0 - 2026-10-13
<!-- big -->

### New

- **Claude Code and Codex on your cloud computer.** For a Claude Code or Codex teammate, the chat-type menu now offers Cloud computer beside its maker's own cloud. The turn runs on your own GitHub Codespace, with Claude Code or Codex there, on the model in the box. It streams here like any turn, carries the conversation on, and its change waits there until you apply it here or send it to a pull request.
- **Signed in with one click, and no code to copy.** The first time, Locust puts the agent on the computer and opens its maker's own sign-in page in your browser. You click Authorize, as on this PC, and it is signed in there. Settings > Cloud computers shows which agents are signed in on each computer, with Sign in and Sign out. The sign-in stays on that computer and goes when it is deleted.

### Improved

- **Scout says it is keeping watch.** When nothing needs you, Home now shows one quiet line instead of nothing: Scout keeps watch, when it last looked, and Look now. A Scout that is watching no longer looks like one that is off. Settings says when it last looked too.

## 0.767.0 - 2026-10-13

### New

- **A point can count more.** End a point with ×2 (or x2, up to x5) and it counts that many times in the score. The judges score it like any other point, and the scorecard shows its weight beside it. Every score, a comparison's and a benchmark's, adds up the points met by their weight.

## 0.766.0 - 2026-10-12

### New

- **Draft the points.** Under a comparison, while the box for what a good answer does is empty, Draft the points asks the judge you picked to write three to five checkable points from your question, without answering it. They land in the box for you to edit before the judges read. Three judges vote on the points you write, so this gets you there in one press.

### Improved

- **A tall box of points keeps its row tidy.** The judge's model and Judge now line up with the top of the box instead of floating to its middle.

## 0.765.0 - 2026-10-12

### New

- **Three judges vote.** One judge can be wrong. Under a comparison, choose Ask three judges to vote, write the points a good answer meets, and three models score every answer on their own. A point counts when two of the three agree, and each one shows the vote as a dot for each judge. If a judge does not answer, the others still vote. A benchmark judged by three is scored by the same three on every run again. Free models that are not in the comparison are offered first.

## 0.764.0 - 2026-10-12

### New

- **Your benchmarks, in one place.** Once you have run a comparison again, Benchmarks appears in the sidebar. It lists every benchmark with its question, its models, how many times it ran, which model leads and its score, and when it last ran. A row opens the benchmark at its table, and Back takes you to the list.

### Fixed

- **A benchmark Locust closed on finishes.** If you quit Locust while a benchmark was running, the run under way said "waiting" for good. At the next start it is settled: scored if its judge had finished, or not scored, saying why.

## 0.763.0 - 2026-10-12
<!-- big -->

### New

- **Your own benchmark.** One run can be luck. Once a judge has scored a comparison, run it again, once or three times more, and see whether the scores hold. Each run asks the same models the same question, from the same commit when it changes files, and the same judge scores it on the same points. The runs go one after another and keep going with the window in the tray. Under the comparison, every run reads as one table, a row for each model: its score out of 100, the runs where it met every point, and its time and cost a task, with a chart of score against cost. One press sets a row's model as your teammate's own.

### Fixed

- **An OpenCode turn that never starts says so.** When another OpenCode on this computer had its storage busy, OpenCode could take a message and never begin on it, and the turn sat on "Starting" for good. After a minute with no sign of the model, Locust now asks OpenCode; if the message was dropped, the turn ends saying so, ready to send again.
- **A cloud turn's edit says where it happened.** A file OpenCode rewrote on your cloud computer said "OpenCode did not report the change", right above the card that shows the change. It now says it changed on your cloud computer.

## 0.762.0 - 2026-10-12

### New

- **A pull request from your cloud computer.** A change waiting on your cloud computer can go to GitHub instead of this folder. Open a pull request drafts the message from what you asked; Locust then puts just that change on a branch of its own, committed as you, and opens the pull request. While it is open, your next change there can be added to it. A change that builds on edits only this folder has is refused in words, and nothing is pushed.

### Fixed

- **Files you attach reach your cloud computer.** A picture, a file from outside the folder or a PDF attached to a message there now goes with it. Before, the teammate was told to read files that were only on this PC.
- **The teammate on your cloud computer knows where it is working.** It was given this PC's folder path, went looking for it there, and said so in its reply.
- **A cloud turn's files no longer offer this folder's copy.** While its change is on your cloud computer, a file row still shows what changed, but no longer offers to open, copy or show the file here: those acted on this folder's file of that name, not the one the teammate made.

## 0.761.0 - 2026-10-12
<!-- big -->

### New

- **Your cloud computer.** Pick Cloud in the chat-type menu for an OpenCode teammate, and it works on a computer GitHub runs for you instead of on this PC: a Codespace on your own GitHub account, one for each repository. The first time, Locust says what it costs and sets it up in about a minute. It has 2 CPUs and 8 GB, with 60 free hours a month from GitHub, then $0.18 an hour; with no payment method on GitHub, it stops at the free hours instead. Each turn starts from this folder exactly as it is, including commits you haven't pushed and edits you haven't committed, and streams into the conversation like any turn. Its change waits there until you choose Show the change, Apply to this folder, or Discard, and nothing the teammate does there reaches GitHub. Settings > Cloud computers lists them, with a terminal onto each, Stop and Delete.

## 0.760.0 - 2026-10-12

### New

- **The judge scores each answer, point by point.** When you ask a judge in a comparison, write what a good answer does, one point a line, and the judge marks every answer met or not met on each point, with its reason in a line. Above its view comes a table: each answer's score, how long it took and what it spent. Write no points and the judge sets three from your request, and says they are its own.

## 0.759.0 - 2026-10-12

### New

- **Muse Wonder for a Muse Code teammate.** Meta's newest model came out today. Pick Muse Wonder 1.0, or Muse Spark 1.3, for any teammate on Muse Code, with Muse Code's own effort levels. Muse Code also runs Muse Wonder when no model is picked.

### Fixed

- **A Muse Code turn reads cleanly.** Its searches and reads show as steps by name, a model call Muse Code tries again says so once, and its own bookkeeping no longer shows as "Unhandled Muse record" lines in the conversation.

## 0.758.0 - 2026-10-12

### Improved

- **A turn that goes quiet says so.** When a teammate's turn has said nothing for five minutes, its line says "no word for 6 min", with Stop beside it. A command that takes a while, like a test suite, still shows what it's running, without the note.
- **Since you were away says what each run came to.** Each row on Home now has a second line: the first line of what the run answered, or why it failed. A routine that ran overnight tells you what it did before you open it.
- **Scout tells you about a pull request that's ready and waiting, and about new issues.** A pull request of yours that passes its checks, has nothing asked of it and has waited three days gets one card: merge it, or close it. Issues opened this week on the folder's repository get a card each, three at most per look.

## 0.757.0 - 2026-10-12

### Fixed

- **A free model OpenCode turns away says so, and offers the next one.** When OpenCode's free service refuses a free model (on October 12 it refused every one of them for a while), Locust now says that's on OpenCode's side and offers the next free model in one press. It used to tell you to sign in, which free models never need. New conversations also stop starting on the refused model for a while.
- **A teammate who just finished says so.** Her card on Home says "last worked just now" the moment a run ends, and a finished conversation is dated by when it ended rather than when it was sent.

## 0.756.0 - 2026-10-12
<!-- big -->

### New

- **Scout, a teammate that keeps watch.** Turn it on from Home and Scout joins your team. Every 10 minutes it looks at your pull requests and their checks, the reviews asked of you, your main branch's checks, a teammate waiting on you and a turn that failed, then tells you on Home what needs you. Each card says what happened in Locust's own words. Scout adds its take and drafts the next step for the right teammate: Send to them, change it first, or dismiss it. It never sends, merges or changes anything on its own. It reads with the steadiest free model listed (Space Bunny through OpenCode), and only when something is new; when that model's provider won't answer, it moves on to the next free one. Change its model, what it watches, how often it looks and whether it tells you in Windows in Settings > General > Scout keeps watch.
- **Polish overnight, as a routine.** A new starter routine for your team: one teammate runs the project's tests, another fixes what fails, and a third checks the fixes and writes a report for the morning. The fixes are made in a copy of the folder that waits for you to keep or discard. Set it to run each night from Routines.

### Fixed

- **Your saves go through when Windows holds a file for a moment.** Teammates, routines, memories, rooms, approval rules and every other setting Locust keeps now try again for about two seconds when another program, usually the antivirus, holds the file, instead of losing the save.

## 0.755.0 - 2026-10-12
<!-- big -->

### New

- **Start Talk from any app.** Press Ctrl+Alt+Space anywhere, even with Locust minimized or in the tray. Locust comes up and starts Talk: with the teammate whose conversation is open, or with ChatGPT's voice on Home. Press the keys again to end the call. Change the keys or turn them off in Settings > General > Talk from any app. They work once Talk is ready on this computer, and Settings says if another app already uses them.
- **Each Talk call sums up what it handed out.** Every to-do a call starts shows who has it, what was asked and how it's going (Working, Needs you, Done, Failed), as it happens. When a call from Home ends, its summary comes first, with Open beside each to-do. A call with a teammate that handed out more than one thing, or something to another teammate, keeps its summary in their conversation, where the call ends.

## 0.754.0 - 2026-10-12

### New

- **Ask your team from your chat apps, through OpenClaw.** Settings > General > Let your other AI apps use Locust now gives OpenClaw's line too, beside Claude Code's and Codex's. Run it once, and a message you send OpenClaw from WhatsApp, Telegram, iMessage or Slack can ask your Locust teammates. OpenClaw checks the connection before it saves it.

### Improved

- **Other apps can read without asking each time.** Listing teammates, routines and background runs, and reading a reply, are now marked as read-only, so OpenClaw and Codex run them without a question. Starting a conversation, sending a message and running a routine still ask.

### Fixed

- **A save Windows holds for a moment is tried again.** When another program, usually the antivirus, held a file Locust was saving, the save of a background turn's return to its conversation was dropped. It now tries again for about two seconds, as do saved calls.

## 0.753.0 - 2026-10-12

### Fixed

Twelve bugs from the second bug hunt, each with a test.

- **The Changes panel lists every changed file.** Images and other binary files, renamed files, files made executable, new empty files and new files too big to show were missing from its list, tree and totals. When more new files exist than it shows, it now says so.
- **The Changes panel reads more repositories right.** A file with an accented name is named as it is, not as git's escapes and not as a rename. A repository with no remote whose main branch is master shows the branch's changes. A picked commit that left the branch (after an amend or rebase) shows all the changes instead of "Nothing has changed here".
- **Add a connector no longer replaces one you already have** in Gemini CLI, OpenCode or Antigravity when the agent can't list its connectors. It leaves that agent alone and says why.
- **A long call with a teammate is kept.** A call where you asked more than 40 times was lost whole. A calls file that can't be read is no longer replaced by the next call, and a call that can't be saved says so instead of claiming it was kept.
- **A link with a stray % in a reply no longer breaks the window,** for example a Notion page called "Launch plan 100% done".
- **Every conversation waiting on a teammate's reply hears when it can't start,** not only the first.
- **Deleting a conversation frees its pin.** Pins of deleted conversations still counted toward the 50.
- **Update for the GitHub CLI on Windows carries on when winget has nothing newer,** instead of saying it failed.
- **A call's own connection for teammates always closes when the call ends,** even when the call ends while it is opening.

## 0.752.0 - 2026-10-12

### Improved

- **ChatGPT's voice has a face on Home calls.** In a call with a teammate, the call bar shows the teammate's face. On Home it now shows an orb, a dotted sphere that swells with the voice, ripples while ChatGPT speaks, and brightens when it does. It replaces the five level bars. The design is after "Halftone Sphere" from Book of Shapes.

## 0.751.0 - 2026-10-12

### New

- **Hear a voice before you pick it.** Hear it, beside Talk's voice in Settings and beside a teammate's voice, plays that voice saying hello. The first play takes a few seconds on your ChatGPT plan, and Locust keeps the greeting, so every play after that is instant.

### Improved

- **A ChatGPT app's question names the app.** On a call, a question from one of your ChatGPT apps reads "Gmail: send email" instead of "ChatGPT: gmail.send email".

## 0.750.0 - 2026-10-12

### Improved

- **The conversation you're on a call in says so in the list.** Its row reads "on a call" until the call ends.

### Fixed

- **"Started from a voice call" now really marks only conversations a call began.** 0.748 said so, but a conversation you typed still carried the label once a call joined it.

## 0.749.0 - 2026-10-12

### Improved

- **On a call with a teammate, Allow is a card in their conversation.** When a connector wants to send or change something during the call, it asks on the same card as every other approval in Locust. The card shows what it would do and each value it would send, with Allow once and Deny. Saying yes out loud doesn't allow it. The call bar shows Waiting for you to allow it, and Show takes you to the card.

## 0.748.0 - 2026-10-11

### Fixed

- **A teammate's voice no longer guesses.** It could answer before the work came back, then correct itself. It now says it's checking and answers only once the teammate has reported.
- **Lines from a call appear in the order they were said.** A long answer used to land after the reply that interrupted it. Words still being said when the call ends are kept too.
- **The call bar no longer covers the newest line.** The conversation leaves room for the bar while you're on a call.
- **"Started from a voice call" marks only conversations a call began.** A typed conversation that a call joined later no longer carries the label.
- **A call's length counts from when it connected,** matching the clock on the call bar.

## 0.747.0 - 2026-10-11

### New

- **A call with a teammate lives in their conversation.** What you and the teammate say appears in the thread as you talk, marked as said, around the work the call asked for. When the call ends it folds to one line, like "Call with Ada · 1:43 · 4 lines said", that opens again. It's kept with the conversation after a restart.

### Fixed

- **A call no longer asks you to approve things that need no approval.** The voice could say "I need your approval in the app" when there was nothing to approve. A call can't run commands, open a browser, or use your computer: it reads your folder with Locust's own tools and hands work to teammates. Connectors that would change something still ask first.

## 0.746.0 - 2026-10-11
<!-- big -->

### New

- **Talk to a teammate.** Press Talk in a teammate's conversation and you're on a call with that teammate. The voice speaks as them, their face is in the call, and what you ask goes into their conversation. They do the work on their own AI agent, in their own mode, with their approval cards in the thread. On Home, Talk is still ChatGPT's own voice and hands work to whoever fits.
- **Each teammate can have its own voice.** Pick one of ChatGPT's nine in the teammate's settings. Without one, they use the voice in Settings > General.

## 0.745.0 - 2026-10-11

### New

- **Choose Talk's voice.** Settings > General > Talk's voice offers ChatGPT's own nine voices, under the names and descriptions ChatGPT gives them: Juniper, Maple, Spruce, Ember, Vale, Breeze, Arbor, Sol and Cove. The next call speaks in the one you pick.

## 0.744.0 - 2026-10-11

### Fixed

- **An OpenCode teammate in Ask mode can review your changes.** `/review` and questions like "what changed?" used to fail, because Ask mode refused every command, `git diff` included. Git commands that only read now run: status, diff, log, show, blame, grep, listing branches. Everything else stays refused, including git's flags that write files and commands chained after git.

## 0.743.0 - 2026-10-11

### Improved

- **Settings reads cleaner.** Every page opens with its name, so no heading further down outranks the settings above it. The long network paragraph on Privacy is one line, with the rest under "What reaches the network". The steps for adding Locust to Claude Code or Codex by hand are folded away. GitHub and your connectors sit as rows like the rest of Settings, not boxes. Labels that were in code capitals are plain words.

## 0.742.0 - 2026-10-11
<!-- big -->

### New

- **Talk can use your connectors.** In a voice call, ask for quick things directly, like "check my email" or "what's on my calendar". Connectors that only read answer straight away. One that would send, change or buy something shows Allow and Deny in the call, and nothing happens until you press Allow. Bigger jobs still go to a teammate.

### Fixed

- **A voice call reads your folder now.** It used to try commands it was not allowed to run, then say it was waiting for an approval that never appeared. It now reads, lists and searches the folder with Locust's own read-only tools, and it never reads a file of secrets such as `.env`.
- **A voice call never runs programs on your computer by itself.** Add-ons that run code on your machine stay off during a call; online connectors stay on.

## 0.741.0 - 2026-10-11

### Fixed

- **Adding Locust to the Claude app is safer.** If Claude changes its settings at the same moment, Locust starts over and keeps both changes. Locust never touches another add-on you named "locust". The copy it keeps is always your original settings. If Locust has moved, the line offers Update. The Restart Claude prompt is still there after Locust restarts, and it also appears after Remove. Restart closes only the Claude app.

## 0.740.0 - 2026-10-11

### New

- **Add Locust to the Claude app in one press.** Settings > General > The Claude app: press Add, and a Claude chat can ask your Locust teammates and read their replies. Locust makes the change for you and keeps a copy of Claude's settings first. If Claude is open, a Restart Claude button finishes it. Remove takes it out again.

## 0.739.0 - 2026-10-11

### New

- **Talk can hand work to your teammates.** In a voice call, say what you want done and who should do it ("have Wren fix the login bug"). The voice says it back, and once you say yes it starts a real conversation with that teammate, in the teammate's own mode and with its approval cards in the window as usual. The call shows "Handed to Wren", the conversation says it was started from a voice call, and later you can ask the voice how it went.

### Fixed

- **Sign in with GitHub works again** with the newest GitHub CLI, which words its one-time code differently.
- **After Locust updates Codex, its new models appear in the picker right away**, without a restart.

## 0.738.0 - 2026-10-11

### Fixed

- **A voice call is sealed off more tightly.** Starting Talk no longer wakes up your Codex add-ons just to list them. The call's Codex now starts with every add-on, plugin and hook switched off, and Locust won't place the call if it can't read your Codex settings or if Codex is set to send voice calls somewhere other than OpenAI.
- **End always ends the call**, even while it's still connecting, and closing Locust to the tray ends it too. Pressing Talk twice quickly no longer starts two calls.
- **A call that can't connect says so** after 15 seconds instead of waiting, and one that drops tells you it dropped.

## 0.737.0 - 2026-10-11
<!-- big -->

### New

- **Talk with ChatGPT's voice.** The waveform beside the chat box's microphone starts a voice call: you talk, ChatGPT answers out loud, and what you each say appears as you go. It runs on your ChatGPT plan through Codex CLI, so there is no API key to add, only Codex signed in with ChatGPT. During a call it can read your folder but changes nothing, and your Codex add-ons stay off. When the call ends, one press puts what was said in the chat box for any teammate. On Windows for now.

### Fixed

- **The chat box's buttons no longer overlap in a narrow window.** At about 860 pixels wide the model button covered the folder and the mode; now the model's name is shortened first.
- **When Codex's sandbox can't start, Locust says what to look for:** a Codex process still running in the background, which can happen after the Codex app's window is closed.

## 0.736.0 - 2026-10-10

### Fixed

- **A Codex command that never started says why.** On Windows, while the Codex desktop app is open, Codex's sandbox can fail to set itself up, and every command fails before it starts in any mode but Auto. The turn now says "Codex's sandbox did not start", and the command explains it: quit the Codex app and ask again, or use Auto, which runs without the sandbox.

## 0.735.0 - 2026-10-10

### Improved

- **A command's approval card says "Unknown" once.** When Locust can't tell what an ordinary command sends or whether it can be undone, the card says so in one Effects row instead of two. A command that can reach the network still shows both rows.

## 0.734.0 - 2026-10-10

### Fixed

- **The Changes panel opens fast in a folder with many new files.** It now reads them all at once: 0.8 seconds where it took 7.7 in a folder with hundreds of new files.
- **A setting you can't change says why on the control itself**, such as Start Locust when you sign in on a copy that isn't installed.

## 0.733.0 - 2026-10-10

### Improved

- **Change a status in a teammate's tracker.** Press a status chip in a table to pick another, such as Done or Blocked. The request to update it appears in the message box, ready for you to send, and your teammate changes the tracker and tells you what that means.

## 0.732.0 - 2026-10-10

### New

- **Every change in the folder, in one panel.** Press Changes in a conversation's header to see everything the folder's branch changed since it left main, beside the conversation, as Claude Code shows it. The changed files are listed by folder with how many lines each added and removed, every file's changes are in one coloured scroll, and the branch's commits are underneath. Pick one to see just that commit. The ⋯ menu shows or hides the file list (Ctrl+Shift+Y), folds or opens every file, and refreshes.

## 0.731.0 - 2026-10-10

### New

- **Watch a pull request.** Press Watch beside the pull request in a conversation's header. Every few minutes Locust checks it and tells you when a check fails, someone requests changes or approves, the branch conflicts, or it's merged. The news appears above the message box, or as a notification while Locust is in the background. When there's something to fix, one press writes the request to your teammate in the message box. You still press Send, so nothing runs or costs anything until you do.

### Improved

- **The pull request shows for a branch you checked out from someone's fork**, such as one you're reviewing.

## 0.730.0 - 2026-10-10

### New

- **Settle or snooze a conversation.** Right-click a finished conversation and choose Settle to move it out of the sidebar, or Snooze to hide it for an hour, until this evening, until tomorrow morning or until Monday. It comes back by itself when something new happens in it or the snooze ends. A search still finds it, and Bring back on its menu returns it to the list. Nothing is deleted.

## 0.729.0 - 2026-10-10

### New

- **Pin conversations to the top.** Right-click a conversation and choose Pin to top. Pinned conversations stay at the top of the sidebar under Pinned, newest first, with everything else under Recents. They stay pinned after Locust restarts. Unpin from the same menu.

## 0.728.0 - 2026-10-10

### Improved

- **A teammate's tracker reads at a glance.** When a reply has a table with a Status column, each status is a coloured chip: done in green, in progress in blue, waiting in amber, blocked in red, not started in grey. Anything else in the table reads as before.

## 0.727.0 - 2026-10-10

### New

- **What a teammate publishes shows as a card.** When a reply links to something a teammate made elsewhere, such as a Claude artifact, a Google Doc, Sheet or Slides, a Notion page, a Figma file or a gist, a card under the reply shows its name, what it is and where it lives, with Open. A bare web address of one of these now opens from its card too.

## 0.726.0 - 2026-10-10

### Improved

- **Update the GitHub CLI from Locust.** When GitHub has released a newer GitHub CLI than the one on your computer, Settings › Connectors says so and offers to update it, with winget on Windows or Homebrew on a Mac. Your computer may ask you to allow it.

## 0.725.0 - 2026-10-10

### Improved

- **Changes are in colour too.** A file a teammate changed now shows its code coloured by language, like code in a reply. Keywords, strings, functions and comments each have their own colour, on both the added and removed lines, and the words that changed are still marked.

## 0.724.0 - 2026-10-10

### Improved

- **Install the GitHub CLI from Locust.** If your computer doesn't have the GitHub CLI, Settings › Connectors now offers to install it, with winget on Windows or Homebrew on a Mac that has it. Your computer may ask you to allow the installer. "Get it from GitHub" is still beside it, and if the install can't run, the card says why.

## 0.723.0 - 2026-10-10

### Improved

- **Less clutter.** A teammate with no fixed model no longer says "runs on the model you pick" on every card. A route on your account's default model now reads just "Codex" or "Claude", not "Codex / Account Default", in the message box, the conversation header and the lists. The conversation list shows what a conversation cost you, or that it was free, instead of token counts, which are still in Details. Headers drop lines that said nothing, such as "all readable".

### Fixed

- **Every agent's limits read the same way.** Codex's limits said "33% used" while every other agent's said "% left". They all say what's left now.
- **A teammate's Recent work lists conversations.** It used to list each message of one conversation, such as "Good, ship it", as if it were separate work.

## 0.722.0 - 2026-10-10

### Improved

- **OpenCode teammates can't close your browser either.** OpenCode teammates now get the same check as Claude Code, Codex and Copilot teammates before every command, in every mode. If a command would end a program you are running, such as your browser, it is stopped and the teammate is told why. Nothing is added to your OpenCode settings, and your own OpenCode plugins still load beside it. If Locust can't start the check, the command runs as it would have without it.

## 0.721.0 - 2026-10-10

### Improved

- **Copilot teammates can't close your browser either.** Copilot teammates now get the same check as Claude Code and Codex teammates before every command, in every mode, Approve each included. If a command would end a program you are running, such as your browser, it is stopped before anyone is asked. Nothing is added to your Copilot settings. If Locust can't start the check, the command runs as it would have without it, rather than every command being refused.

### Fixed

- **No more "Unhandled Copilot record" lines under Copilot's answers.** The newest Copilot CLI sends a few new kinds of update, and each one used to add a line under the answer. They are read quietly now.

## 0.720.0 - 2026-10-10

### Improved

- **Codex teammates can't close your browser either.** Since 0.717 a Claude Code teammate is stopped before it ends a program you are running, such as your browser, by name or by id. Codex teammates now get the same check before every command, in every mode. Nothing is added to your Codex settings. If Codex would also run hooks of its own for that folder, such as yours, a project's or a plugin's, Locust leaves its check out of that run, so a hook you never reviewed doesn't run alongside it.

## 0.719.0 - 2026-10-10

### New

- **Connect your GitHub.** Settings › Connectors starts with GitHub: who you are signed in as, with GitHub's mark. Sign in with GitHub needs no terminal. Locust starts the GitHub CLI's own sign-in and shows its one-time code, and one press copies the code and opens GitHub's page in your browser. Locust keeps no token; the GitHub CLI keeps the sign-in where it always does.
- **The pull request, in the conversation's header.** While the folder's branch has a pull request on GitHub, the header shows its number, whether it is open, a draft, merged or closed, and a dot for how its checks stand. Hover for the whole line; press it to open it on GitHub.

### Improved

- **Code in a reply is in colour.** Keywords, strings, numbers, functions and comments each have their own colour, in 31 languages, read with the same grammars VS Code uses. A block is coloured once it is finished, so nothing changes shape while you read it. A file you open from the conversation is coloured the same way, by its name.
- **A teammate may close the test browser it started, found by its own profile.** Ending a browser by name is still refused. A command that finds the browser by name and narrows it to the teammate's own command line, such as its own profile folder, now runs when every window it matches is one a teammate started. If the filter also matches one of yours, it is refused and the teammate is told which. Process ids written into a loop, such as `for p in 101 102; do taskkill /PID $p; done`, are now checked too, the same as ids written one at a time.

### Fixed

A sweep through Locust's code turned up 24 mistakes; each is fixed and has a test so it stays fixed.

- **Copilot teammates use the effort you pick.** The effort setting was not reaching Copilot, so every Copilot run used its default.
- **A skill from a folder inside a GitHub repository installs from that folder**, not from the top of the repository.
- **A message held for a busy teammate is never lost.** When two messages were waiting for the same teammate, only the first one reached it. Now each one does, and the conversation says so if a message cannot be delivered.
- **Long answers and long waits come back whole.** A long Antigravity answer could stop partway through. A timer Antigravity set on a task that had already ended kept the run open. A Copilot run that reaches its turn or length limit while you work in it now says so and goes on to the messages you queued.
- **Commands that end every copy of a program are named wherever they hide**: inside brackets, inside `$( )` or backticks, in a loop, or behind `timeout` or `nice`.
- **Smaller fixes:**
  - A very long prompt typed in the terminal is still read back.
  - Very long status lines from Muse and Copilot are cut short.
  - Teammates can reach a project's parent git folder on every platform.
  - A task is matched to the room post that names it, even with punctuation in the way.
  - A routine time that passed during a run across midnight is counted as missed.
  - One damaged post no longer hides a whole room.
  - A background Claude turn that is still working is shown as working.
  - Empty Word and PowerPoint text boxes no longer swallow the words after them.
  - A final line too large to keep no longer hides the answer before it.
  - A check that cannot start no longer leaves a timer running.
  - Allow rules ignore letter case only for Windows folders.
  - Runtime updates compare beta versions correctly.
  - A memory that cites a web address is no longer marked out of date.
  - A saved record keeps code blocks exactly as they were written.
  - A workroom's "+N more" counts only real entries.
  - Antigravity projects on Mac and Linux keep their leading slash.

## 0.718.0 - 2026-10-10

### Fixed

- **No teammate closes your browser by its process id either.** 0.717 stopped a Claude Code teammate from closing every window of a browser by name. The same night, a teammate that was stopped that way looked up the browser's process ids and closed it by number instead. Locust now checks process ids too. A teammate may end only what a teammate started, such as a test browser of its own. It can never end your browser, your editor, Locust itself or another teammate's agent. The refusal also tells the teammate to give its test browser a profile of its own, since a test browser sharing your profile is usually why it wanted yours closed.

## 0.717.0 - 2026-10-09

### Improved

- **No teammate closes your browser.** When some models clean up a browser they opened for a test, they end every Chrome or Edge on the computer, closing your windows too. A Claude Code teammate can no longer do that, in any mode, Auto included. Locust stops a command that ends a browser, Node, Locust or another AI agent by name before it runs, and the conversation says what was stopped. Ending what the teammate started itself, by its process id, still works.

## 0.716.0 - 2026-10-09

### New

- **Add a connector to every agent at once.** Settings › Connectors › Add a connector takes a name and the command that starts an MCP server, or its web address, and adds it to each agent you choose: Claude Code, Codex, Gemini CLI, Copilot, OpenCode and Antigravity. Locust runs each agent's own command for it, the way you would type it, and shows what each one said. An agent that already has a connector by that name keeps its own. Undo takes it out again, except from OpenCode, which has no command for that; Locust says where its copy is. Cursor Agent has no such command yet, so add a connector to it in Cursor.

## 0.715.0 - 2026-10-09

### Improved

- **Lists show you what needs a look.** In Conversations and on a teammate's card, a finished conversation is no longer marked in blue on every row. Only one that is running, was stopped short, or was not fully saved keeps its colour.
- **Team cards line up.** A teammate's role stays on one line beside the card's buttons, and Last run no longer breaks over two lines, so the cards in a row are the same height.
- **Limits in Settings are small meters.** Settings › AI agents shows each limit window as a short bar with its figure, and only a window that is close to its limit is amber. The full wording, with when each one resets, appears when you hover.
- **A connector that works takes one line.** Settings › Connectors no longer says "Connected just now." under every connector that is working. The ones that need you still say what to do.
- **Usage is in the command palette.** Press Ctrl K and choose Usage. The Go to list now runs in the order of its shortcuts.
- **A chosen option in a row of options no longer has a bright edge on one side.**

## 0.714.0 - 2026-10-09

### New

- **Usage, across every agent and model.** A line under your accounts on Home now shows the last 30 days: turns, tokens, what was spent, and a bar for each day. Press it to open Usage. Usage does what Claude Code's /stats and /usage do, for every agent at once. It shows a twelve-week grid of the days you worked, with your streaks and busiest day, and how close each account is to its limits. It also lists each model with its share of your turns, its tokens, how much came from its cache, and what paid for it: your plan, your account, a free model, or what an API key spent. Choose the last 7 days, the last 30, or all time.

### Improved

- **A PDF you attach shows as a page, and opens in Locust.** It was shown as the path Locust copied it to. It is now a card with its first page, its name and its page count, and pressing it shows every page beside the conversation. A PDF attached before 0.713 is read the first time you open it.

## 0.713.0 - 2026-10-09

### Improved

- **An attached PDF can be read by every agent.** Locust now opens the PDF itself as soon as you attach it: its text goes into one file, and each page becomes a picture, both in Locust's own folder beside the attachment. Your teammate is told where they are, so it no longer needs a PDF tool of its own. On a tester's Windows computer, Codex's PDF tool needed Python packages that weren't installed, and its sandbox stopped it from installing them, so it took 1 minute 39 seconds and four failed commands to decode the file by hand. The page pictures keep what text cannot, such as equations, tables and charts.

### Fixed

- **A quote in a reply is drawn as a quote again, and the math inside it is drawn.** Since 0.233, a reply's quoted lines (`> `) were shown as a plain paragraph. So an equation inside a quote, which is where Codex puts "a version you could write down", came out as raw `\begin{bmatrix}` and `\sum` line by line. Quotes now have their margin rule back, and what they hold is drawn like the rest of the reply: equations, lists and code. An equation written in the middle of a line, as `\[ ... \]` or `$$ ... $$`, is drawn too.

## 0.712.0 - 2026-10-09

### Improved

- **A new person's first message goes to a free model that answers.** "Use a free model" started on the first free model OpenCode lists, which is simply the alphabet: this week that was a model OpenCode had retired, then one its server no longer has, then one whose provider has been down for days, where OpenCode keeps retrying for minutes before it gives up. Locust now starts on the free model that has answered most reliably in its own tests, a failed run offers the next most reliable, and a comparison puts that one beside it. A free model that has answered for you still comes first.
- **The offer under a failed free model says why.** It said "This model is at its limit" whatever the reason. It now says when OpenCode no longer offers the model, or when the model's provider is down for now.
- **Mac copies update with every release again.** Releases 0.701 to 0.711 were published without their Mac disk images, so a Mac copy stayed on 0.700. Each release is now built on a Mac, launched there, run, stopped and updated before its images are added; 0.711 has them.

### Fixed

- **A free model OpenCode no longer has is left out.** A fresh install of OpenCode still lists Fledge Alpha, but OpenCode answers "Model not found" for it, and that is where a new person's first message went. Locust leaves it out, treats any model OpenCode cannot find the same way, and offers to switch to a free model that answers.

## 0.711.0 - 2026-10-09

### New

- **Claude Code teammates can search the web and open web pages.** In Auto they do it on their own. In every other mode each search or page waits for you on a card that shows the search words or the page's whole address, and runs only if you approve it.

### Improved

- **What it may do says, for every agent, whether it can use the web.** Codex, Copilot, OpenCode and Muse Code search and open pages with their own tools in every mode. Cursor Agent and Antigravity do only in Auto, because outside it their own rules refuse those tools, and the panel now says so instead of saying nothing. When Codex opens a page, its step reads "Opened" and the address rather than "Searched the web for" it.
- **When a free model's provider is down, Locust moves on from it.** A run that failed because its free model was unavailable now offers to start again on a free model that answers. For the next six hours, or until that model answers again, Locust stops suggesting it: "Use a free model", the second model a comparison starts on, and the next model a failed run offers all pass it over. It stays in the model list if you want to pick it yourself.

### Fixed

- **A Claude Code teammate no longer stays "working" after it has answered.** If a command it started kept running after its final answer, the run stayed open until that command ended, which could be never. Locust now ends such a run a minute after the answer and counts it as completed.
- **A Skills from GitHub keep that is refused leaves nothing behind.** When a file did not arrive as GitHub lists it, nothing was kept, as intended, but a hidden half-downloaded folder could stay in Locust's skill library. It is now removed every time.

## 0.710.0 - 2026-10-09

### New

- **Skills from GitHub.** Settings > Teammates > Skills from GitHub keeps skills from a public repository: paste `owner/repo` or its github.com link and press Look. Locust lists every skill in it with what it is for and every file it carries, and names the files that run, such as scripts, before anything is downloaded. Tick the ones you want and press Keep. They are kept exactly as they were at the commit you looked at, and Claude Code teammates use them on every run, Auto included. Nothing changes when the repository does until you press Look for changes and keep again. Kept skills are part of a profile backup.

### Fixed

- **A free model OpenCode has retired is no longer offered.** OpenCode retired Exo Free while still listing it, so "Use a free model" could start a new person on it and their first message failed, and Compare could put it beside your model as a column that never answered. Locust now leaves out a free model OpenCode says is retired, and a run that hits one offers to switch to a free model that answers.
- **A large folder no longer shows files that did not change.** In a folder with more than 5,000 files that is not a Git repository, a file one run never touched could be listed under "changed in the folder while it ran", as added or edited. Locust now counts a file only if it was written while the run went.

## 0.709.0 - 2026-10-08

### Fixed

- **Free models show up as soon as Locust is ready.** With Antigravity installed, every agent's model list waited for Antigravity's usage reading, which can take several seconds. OpenCode's free models reached the model picker about 14 seconds after start, and Home's "Use a free model" was missing until then. The list no longer waits; Antigravity's usage appears when its reading arrives.
- **Locust runs on Electron 44.7**, with the latest security fixes from Chromium.

## 0.708.0 - 2026-10-08

### Fixed

- **Locust runs on Electron 44.6.** That brings six weeks of fixes from the Electron and Chromium teams: a crash after a very large number of messages between Locust's window and its engine, a crash when a file dialog's window closes at the same moment, a window called unresponsive just after the computer wakes, and security fixes from Chromium. It also ends a rare error in the background error log when a window opens.

## 0.707.0 - 2026-10-08

### Improved

- **The other-apps setting reads more clearly.** It says other apps can run your routines too, not only ask teammates, and "Use each teammate's own mode" now sits indented under it as part of that setting, instead of looking like more of the sentence above.

## 0.706.0 - 2026-10-08

### New

- **Each teammate can have its own instructions.** Open a teammate's Edit (or make a new one) and write them in the Instructions box: "Answer in short bullet points", "Ask before adding a dependency". Every turn that teammate takes is given them, after any group's instructions. Empty the box to remove them.

## 0.705.0 - 2026-10-08

### New

- **Pause a routine.** A routine on a schedule has a Pause button beside Edit. Paused, it stops going on its own: its schedule chip says paused, the Routines header counts it apart, and nothing is recorded as missed. Run still runs it whenever you want. Resume, and it counts from that moment, so a routine paused for a week does not fire the instant you resume it. Your other AI apps cannot run a paused routine.

### Fixed

- **When OpenCode gives up on a provider, its words are said once.** A free model's provider that was down showed "Endpoint is unavailable" twice, in a note and again on the red card right under it. Now the card says it.

## 0.704.0 - 2026-10-08

### New

- **Your other AI apps can run your routines.** With "Let your other AI apps use Locust" on, an app such as Claude Code or Codex can list your routines (each step's teammate and mode, and what each asks for), run one with its answers filled in, and follow it step by step to its last answer. It runs exactly as Run on the routine's card does. A routine that changes files runs from another app only if "Use each teammate's own mode" is on, and its approval cards still appear only in Locust's window. A routine that asks for a folder runs only from Locust, where you pick the folder.

## 0.703.0 - 2026-10-08

### New

- **Your other AI apps can start teammates in their own mode, if you turn it on.** Settings, General, "Let your other AI apps use Locust" has a second switch, "Use each teammate's own mode", off by default. Off, a turn another app starts is Ask (read only), as before. On, it runs in the mode the teammate is set to in Locust, so a teammate on Edit can change files. Its approval cards still appear only in Locust's window, and only you can answer them; the other app is told a card is waiting. Auto still runs only if Auto is on in Settings, and there are still no automatic hand-offs. list_teammates says which mode each teammate's turn would use.

### Improved

- **Scrolled up while a teammate works, the way-down button shows the thinking dots.** You can see the run is still going without scrolling down to check. Point at it, or tab to it, and it is the arrow again. It also keeps its round shape in a long conversation, where it had been squeezed to half its height.

### Fixed

- **Cloud on a Codex teammate says it runs on the model Codex Cloud chooses.** Codex Cloud takes no model of its own, so the model in the box did not decide it.

## 0.702.0 - 2026-10-08

### Fixed

- **A Claude teammate whose Claude Code isn't signed in says how to fix it, once.** It showed Claude Code's own "Not logged in · Please run /login" twice, naming a command Locust has no place for. It now says to open a terminal, run claude and sign in (with your Claude account, or an Anthropic Console account to pay with an API key), or to add a Claude API key in Settings, Your own models. An API key in your environment is never handed to an agent, so it does not count as signing in.
- **Report a problem opens on Locust's own GitHub page** (github.com/locust-lol/locust), beside its bug and idea forms, instead of the downloads page. Reports by email still go to support@locust.lol.

## 0.701.0 - 2026-10-07

### Fixed

- **Claude on your own API key works in Your own models, and Test says so.** Anthropic's API speaks the same language as the OpenAI-style servers Locust already takes: add it in Settings, Your own models, with the address https://api.anthropic.com/v1, a Claude model such as claude-haiku-5-5, and your key. Its conversations already worked, but Test answered "400 Bad Request" because Anthropic wants one more detail when listing its models. Test now answers that it serves the model and can use tools.
- **Reading a Claude cloud task in a folder Claude Code doesn't trust yet tells you what to do.** Newer Claude Code versions won't bring a cloud session into a folder until you have opened Claude Code there once and said you trust it. Locust only said "Claude Code did not bring the session in". It now says to open Claude Code in that folder once, answer its question, and check again.

## 0.700.0 - 2026-10-07

### Fixed

- **An OpenCode teammate that asks you something no longer gets stuck.** OpenCode has its own way for a model to ask a question, and Locust never answered it. A teammate that asked that way showed "Working…" until you pressed Stop. OpenCode teammates now ask the way every other teammate does, in their reply, with the question card to answer it.

## 0.699.0 - 2026-10-07

### Fixed

- **A starting Codex, OpenCode or Copilot turn says it is starting.** While one of these agents got going, the line under your message still said "Reading the folder", a step that had already finished, and then jumped to "Working". It now says "Starting Codex CLI", "Starting OpenCode" or "Starting Copilot CLI" while the agent boots, as Claude Code and Cursor already did.

## 0.698.0 - 2026-10-07

### Faster

- **Less work while a reply streams.** The light that travels round the stop button was redrawn on every frame your screen shows, 60 times a second or 144 on a fast laptop screen, while the teammates' faces beside it move at 30. It now moves at 30 as well, and looks the same. While a reply streams, Locust takes about 8% of the machine instead of about 9%.

## 0.697.0 - 2026-10-07

### Fixed

- **Claude's models have the names Claude Code gives them.** Claude Code moved its Haiku alias to Haiku 5.5 today, and Locust's model picker still read Haiku 4.5 for Claude while Cursor already showed 5.5. Locust now takes Claude's model list from Claude Code itself, in the same quick check that lists its / commands, so the picker and every model chip use Claude Code's own names and settings, and a new Claude model appears the day Claude Code knows it. Haiku 4.5 is still there under the older versions.
- **An agent installed outside npm's global folder starts like any other.** When a coding agent's command came from a project's own or a separately installed copy, Locust did not recognise its launcher, ran it through cmd.exe instead, and refused any message with more than one line. Locust's own check of new agent releases found this while it tried Copilot 1.0.93; it covers Copilot as well as Codex now.

## 0.696.0 - 2026-10-07

### Fixed

- **A stopped OpenCode turn says to check the folder once.** OpenCode reports a step only after it finishes, so when you stop one early Locust cannot promise nothing ran, and the stop card says to check the folder before sending again. The Send again line under it said the same thing again in other words. Now it is said once.

## 0.695.0 - 2026-10-07

### Fixed

- **A large file changed in a plain folder shows what changed.** In a folder that is not a git repository, Locust keeps a file's text from before a turn only up to 64 KB, so a one-line change to a larger file read "changed · seen on disk", for any agent that does not report its own changes. Locust now compares it against the copy it already keeps for Undo, and shows the lines added and removed.

### Faster

- **A folder too big for Undo no longer slows every turn.** Finding out that a folder is too big to keep a copy of means looking at every file in it, about a third of a second each turn in a large folder, only to reach the same answer. Locust now remembers that answer for ten minutes.

## 0.694.0 - 2026-10-07

### Fixed

- **Claude Code's own commands are back in the / menu.** Since a recent Claude Code update, it no longer lists its commands until the app talking to it says hello first. Locust kept waiting, gave up, and a Claude teammate's / menu showed only Locust's own commands until a turn had run. Locust now says hello the way Claude's own tools do, which starts no turn and spends nothing, and the menu lists Claude Code's commands from the start.

## 0.693.0 - 2026-10-07

### Fixed

- **A Cursor edit to a large file shows what it changed.** Cursor reports each edit together with the whole file, twice, so a one-line change to a large file came to more than Locust keeps from a single step (256 KB). The change was dropped with the rest, and the files card read "changed · seen on disk" instead of "+1 −1". Cursor puts the change itself first, so Locust now keeps the lines added and removed and the change, and still leaves out the two copies of the file.

## 0.692.0 - 2026-10-07

### Faster

- **A Cursor teammate starts about four seconds sooner.** Before each Cursor turn, Locust asks Cursor which connectors it can use, and Cursor takes three to four seconds to answer. That answer was kept for only five minutes, so most turns waited for it again. Locust now answers from the last reading at once, reads a new one in the background, and reads it once as soon as it finds Cursor, so even the first turn of the day does not wait. A connector you have just signed in to is named from the next turn.

## 0.691.0 - 2026-10-07

### Added

- **Ask your Locust teammates from Claude Code or Codex.** Settings > General > "Let your other AI apps use Locust" turns on a small local server those apps can add as a tool. From there you can list your teammates, start a conversation with one, send a follow-up, read the reply, and see background runs. It is off by default, and:
  - it answers only apps on this computer, and only with a key that is locked to your Windows account and replaced every time you turn it on;
  - every conversation started this way runs in Ask mode (read only), whatever the teammate's usual mode, and never starts another teammate on its own;
  - those conversations appear in Locust's history, marked as started from another app, and your monthly limits apply;
  - Locust never changes another app's settings: the switch shows the line to add for Claude Code and for Codex.

## 0.690.0 - 2026-10-07

### Fixed

- **A routine's time that passes while its last run is still going now says Missed.** Locust never starts a second copy of a routine over one that is still running. It used to drop that time without a word. The routine's card now says, for example, "Missed, 08:00 today" with Run now, the same as a time that passed while Locust was closed.
- **Antigravity usage on Home is read only from agy 1.1.11 or newer.** Older versions could send `/usage` to the model as a message and spend a turn. On an older or unknown version, Home leaves the line out.
- **A teammate's own branch starts more reliably.** On Windows, long project paths no longer stop it being made. A setup hook in your repository that fails after the branch is ready, such as Git LFS, no longer fails the teammate's start: Locust checks the branch is really there and usable first, and notes the hook's message in its log.

## 0.689.0 - 2026-10-07

### Fixed

- **A model of your own no longer reads as free against a monthly limit.** A model you add in Settings reports no price, so its runs used to count as nothing, and a teammate with a $5.00 limit showed "$0.00 of $5.00" however much its provider billed. The card now counts those runs on their own line under the dollars, for example "$0.40 of $5.00" and then "2 runs with no price", and hovering that line says why. When you set a limit, it says that these runs are not counted.
- **The voice glow rises with less voice.** Ordinary speech now lifts it about a quarter higher. Its highest point is unchanged, so it never covers more of the chat box than before.

## 0.688.0 - 2026-10-07

### Added

- **Choose how voice typing hears you.** Settings > General > Voice typing has three choices:
  - **Fast**, the one you had: it runs on this computer after a one-time 41 MB download.
  - **Accurate**, which also runs on this computer: a larger model with a one-time 69 MB download. It is slower: a ten-second recording takes about three seconds to type.
  - **Your OpenAI account**, which sends your recordings to OpenAI's transcription model with your own API key. Your OpenAI account is charged for it; a ChatGPT subscription does not include API usage. Locust asks once before the first recording is sent. The key is locked to your Windows account, never shown again, and left out of profile backups.

## 0.687.0 - 2026-10-07

### Fixed

- **The chat box's menus open again.** In 0.686, the chat type, mode and model menus were cut off and showed only as a sliver above the message box. They open fully again.
- **The voice glow comes from the chat box itself.** In 0.686 it was attached to the edge of the window, so it looked like it came from the bottom of the app. It now rises from inside the message box, and the box's focus outline stays visible.

## 0.686.0 - 2026-10-07

### Added

- **The chat box glows with your voice.** While voice typing listens, a soft glow rises from the bottom of the message box and follows how loudly you speak, so you can see it's hearing you. It stops as soon as you stop listening and uses nothing at rest. The glow is the open-source voice-glow library.

## 0.685.0 - 2026-10-07

### Added

- **Antigravity's plan usage on Home.** Antigravity's card now shows how much of your 5-hour and weekly limits is left for Gemini models, and when each resets. It also shows Claude and GPT models when those are lower. Locust reads it from Antigravity's own usage command, which costs no request and doesn't use your login. Claude Code and Codex already showed theirs; Cursor still offers no way to read it without your saved login, so Locust doesn't.

## 0.684.0 - 2026-10-07

### Added

- **Claude can keep working when you close Locust.** Pick **Background** in the chat type menu (Claude teammates only), then send. Claude Code runs the turn in its own background session, so it keeps going if you close Locust or restart.
  - **The panel:** a new **In the background** panel shows each run as working, waiting for you, or done.
  - **Answering it:** when a run needs a yes, **Open it to answer** opens it in Claude Code in a terminal. Locust never answers for you.
  - **When it finishes:** the turn comes back into its conversation under an "In the background" divider, even if it finished while Locust was closed.
  - **What does and doesn't apply:** Locust's approval cards and spend limits don't apply to a background run. Claude Code's own permission mode does, set from the mode you picked (Ask, Edit, Approve each or Auto).
  - **First time in a folder:** Claude Code has to be told once that you trust the folder. Locust offers to open it there for you to answer.

### Changed

- **Antigravity narrates less.** Locust asks it to say what it's doing whenever it starts something new, rather than before every step. On the same task it spoke 3 times instead of 10.

## 0.683.0 - 2026-10-06

### Fixed

A second AI model reviewed this week's new code and tried to break it. These are the fixes it led to.

- **Commit is stricter about what it commits.**
  - It commits exactly the files it showed you. If the folder changed after you opened the panel, nothing is committed and you're asked to look again.
  - The file list can now show every file, not just the first six.
  - A file Locust keeps for itself is never included, even one that was already staged.
  - If your hook refuses the commit, Locust leaves your staging alone when it changed while the hook ran, and tells you so.
  - A branch name that could be mistaken for a git option is refused.
  - A failed checkout hook no longer leaves you on a half-made branch.
  - Pull requests are offered only for remotes that really are on GitHub.
- **The stray-log cleanup from 0.681 is more careful.** It reads every line it would remove first. It never touches a log another program wrote to in the meantime, and never follows a link.
- **A merge Locust couldn't check for conflict markers is no longer called ready to land,** and Land waits until it can check.
- **Opening Locust a second time** no longer clears skill files that a running teammate is using.
- **In Compare:** after you keep a model's answer, a page it made opens from your folder even when the model named it by its full path in its own copy.
- **A review note's label** shows the file's path within the folder, not the whole path.

## 0.682.0 - 2026-10-06

### Added

- **Voice typing (Windows).** A microphone button sits beside Send. Press it, speak, and press it again: your words appear in the message box at the cursor, ready to edit. Nothing is sent until you send it, and Esc cancels.
  - **It runs on your computer.** Nothing you say leaves it.
  - **A one-time download, the first time you press it:** about 41 MB, after you agree. It isn't in the installer, and nothing loads until you use it.
  - **Checked on arrival:** every downloaded file is checked against a known fingerprint, and a file that doesn't match is deleted.
  - **Access:** the microphone is allowed only for Locust's own window, and only audio.

## 0.681.0 - 2026-10-06

### Fixed

- **No stray `error.log` in your project.** When a GitHub folder was open, Locust checked with Codex whether the folder could use Codex Cloud, and Codex left a debug log named `error.log` in the folder. The log includes your ChatGPT account id. If your project doesn't ignore `*.log` files, a commit could pick it up: the first real pull request opened from the new Commit button carried one. Locust now leaves `error.log` exactly as it found it. It removes one Codex made, and trims only the lines Codex added to one you already had.

## 0.680.0 - 2026-10-06

### Added

- **Commit from the conversation.** When a teammate has changed files in your folder, a Commit button appears at the top of the conversation. It lists the files, drafts a message from what you asked, and commits them as you, with your own git name and hooks. You can also push, or open a pull request when the folder is on GitHub and the GitHub CLI is signed in. A pull request from your main branch goes from a new branch, so main stays where it was. If a hook refuses the commit, what you had staged is left exactly as it was. The button only appears when nothing is running, so a commit never takes half a turn.

### Fixed

- **A merge with conflict markers left in no longer says it can land.** When a teammate asked to resolve a conflict replied with a question instead, the thread said "the merge is finished, so it can land now", while the Land card said markers were still there. It now names the files that still have markers.

## 0.679.0 - 2026-10-06

### Added

- **Claude Code teammates can use skills.** A skill in your folder's `.claude/skills` now works for a Claude Code teammate in every mode, the way it does in Claude Code itself. Ask it to "use the review skill" and it does. Before this, outside Auto, a teammate couldn't see any skill, and the request often went to a connector with "skill" in its name instead. A skill uses only the tools the teammate's mode already gives it, so a teammate in Ask that uses a skill still changes nothing.
- **Lend your own skills.** Settings > Teammates > Your skills lets Claude Code teammates use the skills in `.claude/skills` in your home folder too. It is off until you switch it on. Auto already runs as you and finds them either way.
- The thread says "Used the review skill" where it used to show the tool's name. A file the skill reads shows as `.claude/skills/review/…`, not as the temporary copy Locust made for the run.

## 0.678.0 - 2026-10-06

### Fixed

- **Compare lets you pick two models from the start.** The first time you used Compare, with no recent models to start from, the model list closed after your first pick, though it said "Pick two or three models". You then had to find the + to add a second. The list now stays open until you've picked two. The + button also says "Add a second model" when only one is picked, not "Add a third model".

## 0.677.0 - 2026-10-06

### Improved

- **OpenCode replies appear as they're written.** On OpenCode, including the free models, a reply used to arrive all at once when it was finished, often after 20 to 60 seconds of nothing. It now appears word by word, as Claude Code, Codex and Cursor replies do. Every mode means what it meant before: Ask still can't change anything, Edit and Auto still work as they did, and Approve each still asks you first.

## 0.676.0 - 2026-10-06

### Fixed

- **Keeping a comparison no longer leaves branches in your project.** In a Git project, each model in a comparison works on its own branch. When you kept one answer, Locust removed the copies but left both `locust/compare-…` branches behind, so every comparison you kept added two branches to your repository. They're now removed with the copies. Branches left by earlier versions stay until you delete them (`git branch -D locust/compare-…`).

## 0.675.0 - 2026-10-06

### Fixed

- **Compare says where the models will work, before you start.** In a folder too big to copy, a comparison that makes changes works in the folder itself, but the Auto option still promised that each model would work in its own copy. It now says what will happen: each model works in the folder itself, and every model's changes land there, whichever one you keep.

## 0.674.0 - 2026-10-06

### New

- **Undo a turn.** When a teammate changes files in your folder, the end of that turn now offers **Undo these changes**. Locust asks first ("Put back the 2 files this turn changed?"), then puts every file back exactly as it was before the turn: edits reverted, new files removed, deleted files returned. A file you changed yourself since then is left alone, and Locust names it and says why. It works in any folder, whether or not it uses Git, and never touches your own repository: the copies live in Locust's own data, are not included in a profile backup, and are kept for each folder's last 60 turns. A folder with more than 20,000 files or 300 MB of files is not copied, and a file over 10 MB is not kept. Locust doesn't offer Undo for those, or for a turn that shared the folder with another teammate's run at the same time.

## 0.673.0 - 2026-10-06

### Fixed

- **A teammate's eyes no longer shake in the sidebar and the conversation header.** Since 0.654 those faces are drawn 15 times a second to save power, and the spring that lets a screen's eyes trail the body's hop could not take a step that long: it swung the eyes from one end of the screen to the other on every frame. The spring now takes long frames in short steps. Measured in the app on a working teammate: the eyes swung on 30 to 33 of 38 frames before, and on none after.

## 0.672.0 - 2026-10-06

### Fixed

- **A Claude Code edit shows what it changed, in any folder.** Claude Code reports each edit, but Locust never read that report, and worked the change out by looking at the folder after the run. Past what Locust looks at (a file over 64 KB, or a folder with thousands of files, like `.claude`), a Claude edit read "changed" or "did not report the change", with no lines to show. Now each Edit and Write carries Claude Code's own change, line for line.
- **A failed edit stays a failure.** When an agent's first attempts at an edit failed and a later one landed, the list of steps could show the first failure as the change, and the files card could count the file twice ("Edited 1 file · 1 more changed in the folder"). The steps now say which attempts failed, and the card shows the file once, with the change that landed.

## 0.671.0 - 2026-10-06

### Improved

- **Switching agents mid-task carries what was already done.** When you hand a conversation to another agent, Locust tells it what the first one finished. On a long run that list did not fit, so it was left out whole and the new agent knew nothing of the work behind it. Now the most recent finished steps that fit are kept, with a line saying how many earlier ones are not listed. The same goes for the earlier messages of a long conversation.

## 0.670.0 - 2026-10-06

### Improved

- **A conversation uses less of your computer while a reply streams in.** Every moving face was redrawn 30 times a second, including the small ones beside a name, in the sidebar and the conversation's header, which show the same teammate as the face beside the reply. Those now redraw 15 times a second and still move at the same speed; the face you are talking to stays at 30.

## 0.669.0 - 2026-10-06

### Fixed

- **Prompt stands on the title screen's machine again.** Since Prompt was drawn bigger in 0.663, the cover still placed him by his old height, so on a narrower window his feet dipped into the LOCUST panel. He stands on its edge again.

## 0.668.0 - 2026-10-06

### Improved

- **Antigravity says what it is doing as it works.** On a long job, Gemini on Antigravity could work for twenty minutes with nothing on screen but "Working...". Locust now asks it, in one line, to say in a short sentence what it is about to do before each step. Measured on the same task: no words between its steps without the line, one before every step with it. Claude Code, Codex and Cursor already do this on their own and are not asked.
- **Antigravity's answer no longer repeats what it said along the way.** Its last message held every sentence of the turn, so a line it had said before a step came back on top of its answer.

## 0.667.0 - 2026-10-06

### Fixed

- **A message between teammates is never lost to a run that did not start.** When one teammate wrote to another and the reply's run failed before its agent had started (a refused setting, a sign-in problem), the message was already counted as read and was gone. Now it counts as read only once the agent has actually begun, so it waits for the next run instead.
- **"Run it again" no longer turns an automatic reply into your message.** On a run Locust started on a teammate's behalf, it re-ran Locust's own instructions as if you had typed them, and they showed as your bubble in the conversation. Such a run now says the message will be answered on the teammate's next run.

## 0.666.0 - 2026-10-06

### Fixed

- **A model's error reads as a sentence, once.** When a provider refused a run, Locust could show its raw reply, `{"type":"error","error":{"message":"model 'gpt-6.1-sol' is not enabled ..."}}`, in the conversation and again in the red card. It now says who refused and what to do, for example "OpenAI refused the model gpt-6.1-sol: it is not enabled here right now. Pick another model and send again.", and says it once. Usage limits, rate limits, an overloaded service and sign-in problems each get their own sentence. The provider's own words are still kept in the run's record.

## 0.665.0 - 2026-10-05

### Fixed

- **A teammate on Antigravity can answer another teammate on its own again.** A teammate saved with a Gemini model and no effort level started its automatic replies without one, and Antigravity refused them: "requires --effort". Such a reply now runs at Medium, Antigravity's own middle setting, as a run you start yourself already did.

## 0.664.0 - 2026-10-05

### New

- **"step 2 of 4" opens the plan when the plan has scrolled away.** On a long run, the plan card can be far up the conversation. When it is out of sight, the step count on the live line becomes a small button; press or hover it to see the whole plan, where the run is in it, and what is next. While the plan card is on screen, the step count stays plain, so the plan is never shown twice. It reads the plan the run already has and asks no model for anything.

## 0.663.0 - 2026-10-05

### Fixed

- **Prompt and Spark are as big as the other teammates.** The two mascots were drawn smaller inside their box than every other bot, so on a card they looked half the size of the teammate beside them. Both now fill their box the way the others do, and both wear their chest mark, Codex's `>_` and Claude's asterisk, at every size.

## 0.662.0 - 2026-10-05

### Improved

- **Codex Buddy lifts more cleanly on a small face.** His shoulder press now finishes with the weights beside his head instead of over his cap, where they crowded him on a card or in the sidebar, and his hammer curls roll the weights in and out more gently.

## 0.661.0 - 2026-10-05

### Changed

- **Codex Buddy's body is back, and cleaner.** 0.660 took away the body 0.659 gave him; that was a mistake, and it is back: his shirt with the Codex mark, his arms on real joints, his lifts. Touched up: his outlines are a little lighter, the Codex mark a little smaller and still bold, and his curls stay in his picture, so his weights no longer drop out of sight and pop back on every rep.

### New

- **Two agent mascots to pick for a teammate: Prompt and Spark.** Prompt is the Codex mascot, a soft cloud of a head with a screen for a face and `>_` on its chest. Spark is the Claude mascot, a round-petalled star with Claude's mark on its chest. Both wear their screen always, in plastic or plush, and go well with the new Indigo and Coral. A chest mark shows where a bot is big enough to read it; on small faces the screen says enough.

## 0.660.0 - 2026-10-05

### Changed

- **Codex Buddy is drawn as his maker drew him again.** 0.659 gave him a body drawn by Locust below his head; next to his own drawing it was heavier and busier, so it is gone. He is back to his own art, with his screen face, his settle and his breathing.

### New

- **Two more teammate colours: Indigo and Coral.** Close to Codex's blue and Claude's coral, for the agent mascots on their way.

## 0.659.0 - 2026-10-05

### Improved

- **Codex Buddy moves on a body of his own.** Below his head, Buddy is now drawn by Locust: a shirt with the Codex mark, strong arms on real shoulder and elbow joints, and dumbbells that sway in his hands. He presses, curls, flexes when he is done, waves, and tilts his head toward where he is looking. He is framed head and shoulders, so he stays big even when small, and every weight stays inside his square.

## 0.658.0 - 2026-10-05

### Fixed

- **A routine waits for a teammate before it can be saved.** With no teammates yet, a ready-made chain opened in the editor with Save available, and pressing it asked you to choose a teammate from a list that was not there. Save now waits, as the note above it already said: add a teammate first.

### Improved

- **The installer built on GitHub now holds exactly the same app as the one built on our machine.** The only difference was a set of unused helper files that recorded the folder they were built in; they are left out.

## 0.657.0 - 2026-10-05

### Fixed

- **An Antigravity answer that arrived whole is no longer shown as a failed run.** Twice tonight Antigravity finished its whole answer, and only then did its last call to Google come back "service unavailable" (code 503). Locust showed the finished run as "The run could not continue". It now shows the run as finished, with a line saying what happened, and to send "continue" if the answer was not finished. A 503 before any answer is still a failure.

## 0.656.0 - 2026-10-05

### Fixed

- **Switching Antigravity accounts says how.** Sign in again opens Antigravity still signed in, because switching accounts happens inside it. The steps were only in the button's tooltip. Once the window is open, the row now says them: type /logout, then /login, and choose the account.

## 0.655.0 - 2026-10-05

### Fixed

- **A big file's change shows the right line counts.** When a teammate rewrote a long file, the list of changed files could say "+0 −747": the counts came from the part of the change Locust kept, not the whole of it, and read as if lines had only been deleted. A large change now shows the counts its tool reported for the whole change, or, when those are not known, just LARGE with no counts.

## 0.654.0 - 2026-10-05

### Improved

- **Home redraws half as often while a teammate works.** Every moving face on Home is drawn 30 times a second, but each one kept its own count of when, so together they made the window redraw nearly 60 times a second. They now all draw in the same moments: the window redraws 30 times a second, and every face moves exactly as before. (Corrected in 0.657: this first said Home uses less of your computer. Measured since, the saving is too small to show; the cost is in drawing the faces, not in how often the window redraws.)

## 0.653.0 - 2026-10-05

### Changed

- **Codex Buddy's made-up in-between frames are off.** 0.651 drew extra frames between Buddy's own drawings, made on your computer; they were not good enough to keep. Buddy goes back to blending smoothly from one drawing to the next, with his settle and his breathing, and Locust no longer makes or stores those frames. A better way to move him is on its way.

## 0.652.0 - 2026-10-05

### Improved

- **No more "Started in 6.1 s" under every answer.** Each answer began with a line saying how long the AI tool took to start, which in a side-by-side room was often longer than the answer itself. It no longer shows. Locust still keeps it in the conversation's record.

## 0.651.0 - 2026-10-05

### Improved

- **Pets move like what they are.** Every pet's screen now flicks on like the teammates' do. Each pet moves in its own way: heads float and hop, Cabin stays rigid, Macintosh and Bitty rock on their feet, the cat on its laptop never hops and just breathes, and Nori wobbles like jelly. Codex Buddy's lifts are smooth: Locust makes in-between frames from his own drawings, on your computer, once, and keeps them; it does that work only while Locust is open and in front of you.

## 0.650.0 - 2026-10-05

### Fixed

- **Cursor's edits of large files are confirmed.** When Cursor finishes editing a file, its message carries the file's text twice, and for a large file that is more than Locust keeps from any one message. Locust set the message aside, so the edit was never marked done and the list of changed files said "not confirmed" for changes that had been made. Such an edit is now closed and marked "result too large to keep", the way Claude Code's large results already were.

## 0.649.0 - 2026-10-05

### New

- **Copy a change, or the whole file.** A changed file's row in a turn now has Copy beside Open and Show: it copies the whole file as it is now. An opened change has Copy too, at its foot: it copies the code as it reads after the change, without the + and − signs or the removed lines, so it pastes as code. For a new file, that is the whole file.

## 0.648.0 - 2026-10-05

### Improved

- **A teammate at work feels alive.** While a teammate thinks, works or replies, its face leads with the prompt eyes and the hop, then goes round its other faces in between: the thinking dots, a glance to the side, a moment of focus. Each face keeps its own rhythm, so a row of teammates never moves in step. Its screen now flicks on like an old TV when something worth looking at happens: starting work, finishing, getting stuck, or waiting on you. Nothing moves while Locust rests.

### Fixed

- **An Antigravity teammate no longer looks like it is replying through a long run of tools.** Antigravity sends a step's words after that step's tools have started, and Locust read them as the reply, so the face said "replying" for minutes at a time. Words that arrive while a tool is still running are no longer taken as the reply.

## 0.647.0 - 2026-10-05

### Fixed

- **A first launch no longer shows every teammate stuck.** On a fresh install, while Locust was still checking which AI tools are on the computer, a teammate whose tool had not answered yet showed the stuck face, the one that means it has no AI tool it can use. On a slow first check that was every teammate, for many seconds. A teammate whose tool is still being checked now stays at rest, and shows stuck only once its tool has answered that it cannot run.

## 0.646.0 - 2026-10-05

### Fixed

- **A long turn keeps its opening on screen while it runs.** A turn shows at most a set number of its steps while it runs, and past that it kept only its very first event and the newest ones, so on a long Claude Code turn the plan and the model's first messages dropped off the screen until it finished. It now keeps the opening and the newest work, and says so once at the top when it has left steps out ("This turn is long: some work between its opening and recent updates is kept in its record, not shown here"). Everything is still in the turn's record.

## 0.645.0 - 2026-10-05

### New

- **The pets you kept wear Locust's terminal eyes and move like teammates.** Cabin, Astro Bot, Meowbot, Macintosh, Bitty, TmuxAI, Rainbow Terminal Cat and Nori now carry the teammates' eyes on dark glass, with their moods, hops, glances and idle moments. A head on a walking body is shown from the neck up, ears and antennae kept. At rest they draw nothing. This shows while Terminal faces is on; pick As drawn in a teammate's look to keep a pet's own face.

## 0.644.0 - 2026-10-05

### Fixed

- **An edit the AI tool never confirmed says "not confirmed", not "stopped".** A turn's list of changed files said "stopped before it reported" for an edit the tool started and never said it had finished. On a Cursor turn that ran to the end, four edits read that way although they had been made. The row now says "not confirmed", and pointing at it explains that the file may still have changed.

## 0.643.0 - 2026-10-05

### New

- **Ready-made teammate chains: fix a bug, build a feature, make it faster.** Routines > Start from a template now offers three routines whose steps go to different teammates, the last one checking the work: find the cause, fix it, check the fix; plan, build, review; measure, change the slowest part, measure again. Each step names a job rather than a person, and when you start one Locust proposes a teammate for each from your team (a finding step to Research & Briefs, a building step to Code & Migrations, the check to Docs & QA), shown in the editor where you can change any of them before saving. With one teammate, every step is theirs. The run counts only if the checker approves, and its changes wait in a copy for you to Keep or Discard.

## 0.642.0 - 2026-10-05

### New

- **Codex Buddy wears a terminal screen, and lifts in sets.** A teammate drawn as the Codex Buddy pet can now wear the same screen and eyes as Locust's own teammates: his glass over his face, the teammates' eyes on it, blinking as theirs do. While he works he goes round all his lifts in sets (press, curls, squats, bench, pull-ups, a carry); beside a name he keeps to the standing lifts. He waves when you point at him, does a chin-up when he is done, and is stuck under the bench when he cannot run. At the small sizes he is drawn at, he is framed from his cap to his waist so his face can be read. Choose Screen or As drawn in the teammate's look; the screen shows while Terminal faces is on. At rest he draws nothing.

## 0.641.0 - 2026-10-05

### Fixed

- **A comparison still running stays live when its window comes back.** When Locust's window was opened again while a comparison ran (from the tray, or reloaded after an error), a column whose model had not yet reported anything was shown as "interrupted", though the run went on and finished. Every run Locust is still running is now shown as running, and updates that arrived while the window was away are applied in order.

## 0.640.0 - 2026-10-05

### Fixed

- **Test waits for a model of your own, and says what it found.** Tried against a real server, a small model running on a computer's processor took 8 seconds to answer Test's one-word question, and Test gave up after 8: it said the model answered, and nothing about whether it can use tools. Test now gives the model up to a minute and says "Testing…" while it waits. If it still cannot tell, it says so instead of leaving it out.
- **A run is told its folder by its full path.** Every run is told which folder it works in. Told only the folder's name, a small model asked to make a file "in this folder" added that name to the folder it was already in, and wrote the file one folder too deep. Runs are now told the folder's full path, the same one their AI tool gives them, so there is nothing to add to it.
- **Every connection Locust makes is listed, Test's included.** Test asks your model's address which models it serves, then sends the model one capped question. The public list of Locust's own connections, and Settings > Privacy & data, now say so. Settings > Your own models said a key is kept "as Windows encrypts it"; on a Mac the keychain does, so it now says "as your system encrypts it".

## 0.639.0 - 2026-10-05

### Fixed

- **A slow AI tool's models stop saying "checking" once its check is done.** Since 0.634, Locust opens without waiting for the slowest tool, shows that tool as being checked, and updates it when the answer arrives. On a busy computer, when several tools answered late one after another, Locust counted each check that was still running as a tool that would not answer, and stopped asking. When the slowest one, often OpenCode, finally answered, nothing picked it up: its models in the picker said "checking" for good and could not be chosen. A check that is still running no longer counts against a tool, and when a check finishes, Locust reads the stored answer instead of starting the check again.

## 0.638.0 - 2026-10-05

### Fixed

- **A comparison column is told it works in its own copy.** When a comparison changes files in a folder that is not a git project, each model works in its own copy of the folder. Each one was still told it was working in the project folder itself, by name. In one comparison, two OpenCode models made a new folder with that name inside their copies and put their work in it, where Keep and the column's page did not look. Each column is now told it has its own copy, as a teammate with its own branch always was.

## 0.637.0 - 2026-10-05

### Fixed

- **A teammate answers another in the shape it was asked for.** Locust tells a teammate to write its replies to other teammates in complete sentences, and one asked by another for "exactly the word TANGERINE" refused, saying a one-word reply would break Locust's own rules. Now, as when a person asks, the shape the sender asked for wins: one word, a number or a list comes back as that. Run again on the same free model, the teammate replied "TANGERINE".

## 0.636.0 - 2026-10-05

### Fixed

- **A hand-off routine's card keeps its checker in view.** When every teammate in a routine's chain runs on the same model, the card named that model at every step, and the line ran out before "(checks)", the part that says who approves the work. It now names a shared model once, after the chain: "Wren → Atlas → Sable (checks), all on Muse Spark 1.3". Teammates on different models, or on one model through different tools, are still named with their own.

## 0.635.0 - 2026-10-05

### Fixed

- **Settings never cuts a control.** In a narrower window, the On and Off buttons for Terminal faces were cut down to "On" and half an "Off", and they lost a few pixels even at an ordinary width. A setting's words now wrap beside its control, and the control keeps its whole size on every Settings page.

## 0.634.0 - 2026-10-05

### Improved

- **Locust opens without waiting on the slowest AI tool.** At launch Locust asks each installed AI tool whether it is ready, and the window waited for the slowest answer: OpenCode alone took up to 11 seconds on some launches. Now it waits two seconds at most. A tool still answering by then shows as being checked, never as signed out or missing, and its row updates by itself when the answer arrives. Starting a run on that tool waits only for that tool's own answer, and a run on any other tool waits for nothing. OpenCode is asked once at launch instead of twice.

## 0.633.0 - 2026-10-05

### Improved

- **A teammate's face moves as one creature.** A screen face's thinking dots hold their place and the head does the looking, easing toward where it looks instead of darting about. Every change of state is one blink: the new eyes come in under the closed lids, and only then does the body answer.
- **A face says how a teammate is doing.** Waiting on you, it watches you with its head tipped. A message arriving makes it look up. Finishing, its eyes flash green as it hops, then settle back to white. Stuck, it winces, `> <`. A screen face switches on like an old monitor the first time it appears, and its dots blink.
- **A face at rest answers you.** It looks at you while it replies and listens while you type to it. Rest the pointer on a teammate's face and it looks at it. Now and then, while you are using Locust, a resting face has a moment of its own: a look aside, a content squint, a curious tilt, a doze. Behind other windows, after 45 seconds untouched, or with reduced motion on, no face moves on its own.
- **Settings > Appearance shows what each face means.** "What a face says" walks one teammate through every face, each named under it. Click it for the next.

## 0.632.0 - 2026-10-05

### Improved

- **A comparison keeps every column's words in view.** The columns scroll together, so a short answer used to leave its column empty for the whole length of a long one. Now a column that has finished saying everything waits under the names, and a longer one stops with its last words at the bottom, so at the end of the longest answer every column's ending sits side by side.
- **Nothing in a comparison is cut off.** A column's numbers, such as "+212 -4 in 2 files · 1h 09m", wrap between whole parts instead of trailing off. A narrow column keeps the model's whole name and leaves out the word for its tool, which its logo already shows and its hover names. A turn's closing line wraps instead of silently dropping its last part. The live line starts at the column's edge, like the words under it.

## 0.631.0 - 2026-10-05

### Fixed

- **A compare column's live line stays in its column.** With three models side by side, a long step -- "Final regression playthroughs across classes and layouts" -- ran out of its column and over the next one. The line now fits at any width. The step's words shorten with an ellipsis only when they alone do not fit, and hovering shows the whole line. The clock now sits right after the words, the way Claude Code shows it, and is never cut. The note after it, the file or command a step is on, shows when there is room for it to say something and is left out when there is not, rather than shrinking to a stray dot. The same line in a plain conversation reads the same way.

## 0.630.0 - 2026-10-05

### Improved

- **A streaming reply is saved in batches.** A reply arrives in hundreds of small pieces, and Locust wrote each one to the conversation's record, waited for the disk, and sent it to the window on its own: 1,005 writes for a 1,000-piece reply. It now gathers them for at most 50 milliseconds: 45 writes for the same reply, each still on disk before it is shown, in order, nothing lost. A tool starting, an approval card or the run ending sends whatever is waiting first. This does not yet lower what Locust's window costs while a reply streams -- measured, that is the drawing, not the saving -- and that is next.

## 0.629.0 - 2026-10-05

### Fixed

- **A turn's list of changed files counts this folder's files, once each.** A long run that was stopped and resumed showed "Edited 19 files", each row "Claude Code did not report", beside a footer that said one file changed. Writes into a scratch folder outside the conversation's own no longer count as its files, a file written and then edited is one file, and a call the run was stopped in the middle of says "stopped before it reported" rather than that Claude Code failed to.

## 0.628.0 - 2026-10-05

### Improved

- **Locust opens without waiting on Antigravity.** When Locust starts it checks every AI agent on the machine, and the first screen waited for the slowest. Antigravity's check asked its CLI two things one after the other, each allowed a full minute: 3.5 to 5.5 seconds of a normal start, and two minutes plus a false "needs sign-in" when the CLI hung. It now asks both at once on the ten-second clock every other check has; the first screen and every other agent go on without it; its row says it is still being checked, and its answer is filled in when it arrives. A start on Antigravity itself still waits for its own check, and a check that did not answer says so instead of claiming you are signed out.

## 0.627.0 - 2026-10-05

### Fixed

- **A long run no longer goes quiet on screen.** Locust drew only the latest 500 of a turn's events, live and when read back from its record, so once a teammate had made sixty-odd tool calls the messages it had already shown, and the groups of steps between them, dropped off the screen, and the counts went down ("edited 3 files" became "edited 2"). A Sonnet run 35 minutes in read as one bar and silence. Locust now draws 3,000, enough for a 90-minute run at Max, measured at about the cost of a frame to draw. A turn longer still says at its top that its first steps are kept in its record, not drawn. The record always kept everything.

## 0.626.0 - 2026-10-05

### New

- **Pictures show in the chat.** When a teammate reads an image, the picture appears under that step, the way Claude Code shows it. When it makes or changes one -- a chart, a screenshot, a red square -- a thumbnail sits under the file's name in the list of what changed; press it for the full view. An image a reply links with Markdown (`![a chart](chart.png)`) appears in the reply with its caption. Only pictures inside the conversation's own folder are drawn (a comparison column's copy counts as its folder), never from the web or a network path, at most 8 MB, never SVG; a picture that cannot be shown leaves the file's name exactly as it was.

## 0.625.0 - 2026-10-05

### New

- **See what a helper did.** When a Claude Code teammate sends out a helper (its Agent tool), the helper's row now says how many calls it made and opens onto them -- what it read, searched, ran and changed -- one step in, the way Claude Code shows them. They stay out of the teammate's own counts and its live line. A card a helper raises says which helper asked. What a helper reports back reads as words beside its row, without its Markdown marks. Codex and Copilot do not say which calls were a helper's, so their helpers read as before. A conversation's record now marks a helper's calls as the helper's (record version 22); older records read exactly as they did.

## 0.624.0 - 2026-10-05

### Changed

- **A plain chat's ghost is white, like the one on Home.** 0.621 gave a chat with no teammate the ghost with the terminal face, in Locust's lime. The ghost on the Home screen wears its own white; the chat's ghost now matches it, beside every reply and on the live line.

## 0.623.0 - 2026-10-05

### Improved

- **Locust at rest costs almost nothing.** With Home open and nothing happening, Locust kept drawing animations nobody was watching: about 6% of a 12-core machine. After 45 seconds without a key or the mouse, or while its window is behind another, Home now holds the frame it is on; the first touch, or a teammate's work arriving, starts it again at once. Measured with Locust's own probe: Home at rest went from 6.0% to 0.1%. The animations that remain move only by position and fade, which the graphics card draws without the page redrawing itself.

## 0.622.0 - 2026-10-05

### Fixed

- **Keeping a comparison's answer no longer puts your folder in Auto.** A comparison can run in Auto, each model working in its own copy of your folder. When you kept one, the conversation carried on with that model, and opening it read Auto back from the comparison's record, so your next message could run in Auto in your real folder without you choosing it. Keep now carries the model and the effort it ran at; the mode stays your chat box's own, and the bar says which: "You kept Sonnet 5.5 · Max; the conversation carries on with it in Edit, not in Auto as the comparison did, in copies of your folder."

## 0.621.0 - 2026-10-05

### Changed

- **A plain chat wears the ghost with the terminal face.** A conversation with no teammate (a message sent with nobody picked) used to show Locust's own swarm mark beside its replies and on its live "Thinking" row. It now shows the ghost: a dark screen for a face, lit code eyes, the same bot you can pick for any teammate. Colin's call: "he's just so much cleaner and better looking than the locust"; the locust marks stay in the picker and get their own polish later. With Terminal faces off in Settings > Appearance the ghost keeps its own eyes, like every other bot. Teammates you named are untouched; the sidebar and the Board still show a plain chat by its model's logo.

## 0.620.0 - 2026-10-05

### Fixed

- **"Always" on Codex goes through your saved rules too.** 0.616 did this for Claude Code, OpenCode and Copilot; Codex kept its own Always for the rest of the run, so a rule of yours that says no never saw what that Always let through. Now an Always you give a Codex card covers exactly that command, or that change to those files, again; Codex asks about each request, Locust answers from your rules first, and the record says when your Always decided. A Codex request that names neither a command nor a file keeps the old behaviour, and the card says so.

### Improved

- **The documents beside the source describe the product as built.** The architecture page, the project page and the contributor notes published with Locust's source said what was planned in September, not what was built; they now say what runs where, how an agent is gated, what the record is, and what was planned and built differently instead.

## 0.619.0 - 2026-10-04

### Fixed

- **The claude.ai switch reads as one switch.** In Settings > AI agents, the switch that lets you start Claude Code sessions on this computer from claude.ai drew its "Off" sentence in a row of its own, in bright text under a divider, so it read as a second setting. Its state now opens its own line, as on every other switch: "Off. When on, you can start Claude Code sessions in this folder from claude.ai while Locust is open, until you switch it off or quit Locust." While it runs, what Claude Code prints still appears under it.

## 0.618.0 - 2026-10-04

### Improved

- **Every connection Locust makes, written down.** A new page beside Locust's source lists every connection Locust itself makes, when, and what it sends: its own update checks, the AI agents it installs when you ask and keeps current, the pet gallery when you open it, and the libraries a previewed web page may load. It also says what never leaves your computer, and that each AI agent talks only to its own service, with your account. Settings > Privacy & data links to it, and its Network line now names the pet gallery and the agents Locust installs, which it had left out.

## 0.617.0 - 2026-10-04

### New

- **What Locust can stop.** Settings > AI agents now says, for each AI agent, how much of a run Locust can stop. Codex, OpenCode and Copilot ask before each action in Approve each action. Claude Code asks about connector calls, and in Edit about the commands it does not run on its own. Antigravity asks only its questions. Cursor and Muse never ask. Open a row to see what it asks first, what it does without asking in each mode, and who keeps an Always. The same words are in a document published with Locust's source, written from the same list, so the two cannot disagree.

## 0.616.0 - 2026-10-04

### Fixed

- **"Always" no longer outranks your saved rules.** After you answered a card with "Always allow this session", the run's later requests were allowed before Locust read your saved rules. On Claude Code, an Always on one command covered every command after it, including one a rule of yours forbids and one that stops other programs, which Locust says it asks about every time. Now every request goes through one check, in one order: a rule that says no wins, a command that reaches other programs is asked about again (the card says why), and only then does your Always apply. This covers Claude Code, Copilot and OpenCode. Codex still keeps its own Always for the rest of the session, so your rules apply to what Codex asks, not to what its Always lets through; cards from Codex do not say your rules come first.
- **The record says when your Always decided.** A request your earlier Always allowed is now written down with its run, and a saved conversation's record lists it as allowed "by the person's Always on an earlier card of this run", beside the cards you answered and the rules that answered for you.

## 0.615.0 - 2026-10-04

### New

- **Start from a template.** Routines now come with eleven ready-made ones: explain this project, find what is unfinished, review a file, plan a change before making it, challenge an idea before building it, write a status update, a weekly report from your notes, summarize a long text, draft a reply, check a spreadsheet, and find the gaps in a document. An empty Routines screen lists them; once you have routines of your own, they are under Start from a template at the end of the list. Choosing one shows its steps and what it will ask you, and nothing is added until you give it to a teammate. Each one reads and answers without changing a file: it is added in Ask, with no schedule, and runs only when you press Run.

## 0.614.0 - 2026-10-04

### New

- **Back up and restore.** Settings > Privacy & data now copies your teammates, routines, memories, saved approvals and conversations into a folder you choose, and restores them from one. A restore first shows what the backup holds and what it would replace, then restarts Locust to apply it. What it replaces is moved aside in Locust's profile folder, not deleted, so a restore can be undone. Your own model keys are never in a backup. A restore is refused, and says why, while a run is going, for a backup from a newer Locust, or for a backup whose files have changed since it was made.

## 0.613.0 - 2026-10-04

### Fixed

- **A terminal face's thinking dots no longer jump.** The two dots glance about, then bounce in turn like a loader, and at the end of every loop they leapt straight back to where the next one starts. They now drift back, so the loop runs on without a jump.

## 0.612.0 - 2026-10-04

### Fixed

- **Keeping a compared answer keeps its effort.** After you kept one model's answer, the chat box went back to that model's default effort, so a comparison run at High carried on at Medium. The conversation now carries on at the effort the kept column ran at, when the model offers it.

## 0.611.0 - 2026-10-04

### Improved

- **Locust holds still behind other windows.** While another window is in front, every face and every animation in Locust holds where it is, and picks up the moment you come back to it. What a run says keeps arriving the whole time: its text, its steps and its counts. In our measurement, Locust's own work while an answer streamed behind other windows went from about 12% of a 12-core machine to about 3%. The home screen's title characters already rested like this; now everything does.

## 0.610.0 - 2026-10-04

### Fixed

- **A long command no longer stops a Claude Code run.** A teammate that wrote a whole page through one command, a heredoc tens of kilobytes long, was stopped with "the mission ledger could not be written", because the command was longer than a record holds. The command and its description are now cut to fit, keeping both ends, as every other runtime's already were.
- **A Codex command reads as the shell got it.** Codex reports a Windows command with every backslash doubled, so a row read `Get-Content 'C:\\Users\\...\\SKILL.md'`. The row now shows `Get-Content 'C:\Users\...\SKILL.md'`, the command that ran. A command that was not escaped that way is shown exactly as sent.
- **Codex's image tool is said in words.** The live line read "Using imageGeneration"; it now reads "Generating an image", and the steps line says "made an image".
- **A turn's steps read the way Claude Code's do.** The line under each reply said how long the teammate thought and named files mid-line ("Thought for 49s, read 5 files, ran 2 commands, edited storm.mjs, searched hul"). It now says only what was done, in counts when it mixes kinds ("Read 5 files, ran 2 commands, edited a file, ran a search"), and names a file only when that is all it did. A line with nothing else in it still says "Thought for 12s", and a Codex stretch with a headline says just that ("Inspecting the config"). Every name is in the hover and in the rows when you open it.
- **The newest group of steps stays folded, as in Claude Code.** It used to stay open and grow while the run worked, so the latest stretch looked different from every other. It is one line like the rest until you press it.
- **The plan step on the live line has no full stop before the step count.** "Inspect the screenshot. · step 1 of 3" read as two sentences; it is now "Inspect the screenshot · step 1 of 3".

### Changed

- **Home looks like your team.** Each team card puts the teammate's face on a tile of its own colour. While a teammate works, a pill says so ("working", "thinking", "replying") and the line under the name says on what, such as "Editing FirstLaunch.tsx"; while one waits on you the card turns amber and that line says what for, such as "Wants to run pnpm test". An idle card stays calm.
- **The sign says who is working.** While work runs, the glass under the Locust sign names it, such as "Casper waiting on you · Codex working", and goes back to "Autonomous teammates on your own machine" when nothing runs. The three characters on top no longer wear presence dots, which claimed a state nobody was in.
- **Home's lower half reads as one column.** The connected accounts are one quiet line under a hairline instead of a boxed panel that looked like a seventh card. "Compare models" moved to the end of the "Try two models on" row, so Compare is offered once, beside its starters. Your team's heading says how many there are, and every label starts at the same left edge.
- **The first-run sentence is for a first run.** "Your AI agents, on your own accounts. OpenCode works without one." now shows only before you have a team.

## 0.609.0 - 2026-10-04

### Changed

- **The Compare starters are a row of chips.** The one line of links from 0.607 read as a footnote. Each of the three things to try two models on is a chip again: the drawn thing on its tile and its name, one line tall, still a thing to press.
- **The sidebar's places are one row again.** Three rows with a lone number at the far right left the sidebar mostly empty. Conversations, Rooms and Routines sit side by side as they did before the Board joined them, every word whole; Board stays in the bottom bar. The counts are gone: there was no clean place for them in one row, and the list below already shows what each holds.
- **An idle team card is calm again.** "3h ago" and "no work yet" beside the name read as debris. A card says something only when there is something to say: the live word while the teammate works, "waiting on you", or a sign-in it needs. What an idle teammate last did is the card's hover. While Locust is still looking for your agents, no card claims anything.

## 0.608.0 - 2026-10-04

### Improved

- **An Antigravity run's words reach the thread sooner.** While an Antigravity run is live, Locust looks at its transcript four times a second instead of once, so each new step shows up to three quarters of a second earlier. Nothing changes when no run is live.

## 0.607.0 - 2026-10-04

### Changed

- **Home's agent strip is called "Connected accounts".** The line under the sign already said "Your AI agents, on your own accounts"; the strip beneath the team, which lists the agents signed in on this machine, now says what it is from your side of the screen. "7 ready", the marks and "Show all" are unchanged. Settings keeps "AI agents & accounts".
- **The three "Try two models on" cards are one line.** "Try two models on a landing page, a sales dashboard or an arcade game", each a link that fills the box for a comparison, with "Compare models" still beside the accounts. The three cards took a row of Home for a thing most sessions do not start with.

## 0.606.0 - 2026-10-04

### Improved

- **Each team card on Home says where that teammate stands.** Beside the name: "working", "thinking" or "replying" while they work, "waiting on you" in amber when a card is asking, what blocks them in red, and, when idle, when they last worked ("2h ago", read aloud as "last worked 2 hours ago"; "no work yet" before a first conversation). The card used to say "working" or nothing, so a teammate waiting on your answer and one idle since Tuesday read the same.

## 0.605.0 - 2026-10-04

### Changed

- **The sidebar's places are rows, with counts, and Board moved to the bottom bar.** Conversations, Rooms and Routines each take a line of their own with how many they hold at the right (nothing is shown for an empty one). Board sits beside Settings at the bottom: it is a view of the conversations, not a fourth kind of place. The bottom line now reads "7 agents ready"; hovering it still says "7 AI agents ready".

## 0.604.0 - 2026-10-04

### Fixed

- **The steps line under a reply fits its row in whole phrases.** It used to be cut by the row's edge mid-word ("created a-full-rule-store-keeps-every-rule.test.ts, edited ap…"). A long file name is now cut in the middle with its extension kept ("a-full-rule-stor…very-rule.test.ts"), and when the sentence is still wider than the row, names give way to counts ("edited a file, read a file"), then the last phrases to "and 2 more". The full sentence is the line's hover title, and the rows under it still carry every full name.

## 0.603.0 - 2026-10-04

### Improved

- **A streaming reply no longer waits on the disk one piece at a time.** Every piece of a reply is written to the conversation's record before it is shown, and each write is a disk sync. The thread used to wait for each sync before reading the next piece, so on a busy disk (another teammate streaming, a build running) every piece paid its own wait, several hundred times a turn. Now the next pieces are read while one write is on the disk, and go in one write when it lands. On a quiet disk nothing changes; on a busy one the stream keeps pace with the runtime and the writes fall to what the disk can do. What is shown is still only what is already on disk.

## 0.602.0 - 2026-10-04

### Improved

- **While a teammate starts, the thread says what is happening.** Between Send and the runtime's first word there are several seconds (a median 4 to 10 depending on the runtime), and the live line said only "Starting" for all of it. It now says the phase: "Looking for Codex CLI", "Briefing", "Reading the folder", "Starting Codex CLI", in the runtime's own name.
- **When the turn ends, Details records how long each part of the start took.** One line, kept in the saved record too: "Started in 6.3 s: looked for Codex CLI 0.1 s, briefed 1.4 s, read the folder 0.8 s; Codex CLI took 3.9 s to say it had started." A slow start can be read rather than guessed at.

## 0.601.0 - 2026-10-04

### Improved

- **Save the record leaves out text shaped like a key or a token, and says how many pieces it left out.** The saved Markdown is written to be sent to someone, and it carried command output, diffs and the words said with a card's answer exactly as recorded. Each piece shaped like a key (an API key, a GitHub or Slack token, a signed token, a private key, a password written after its name, a card number) is now replaced with "[secret-shaped text removed]", and the file's header says how many pieces were replaced, or "none found". The raw JSON, if you tick it, is the ledger exactly as recorded and says so at its top; the tick now reads "Include the raw record (JSON, not scrubbed)".

## 0.600.0 - 2026-10-04

### Fixed

- **The 201st saved approval rule is refused, never silently dropped.** Locust keeps up to 200 saved rules. Adding one past that quietly dropped the oldest rule, which may have been a deny you relied on. Now the 201st is refused with "200 rules is the most Locust keeps; remove one first." and nothing changes; a rule file that already holds more than 200 is read whole.
- **Home opens a few ledgers at a time.** Listing your conversations opened every ledger file at once, up to two thousand of them, which on a large profile could run out of file handles and read like a corrupt ledger. It now reads sixteen at a time, in the same order as before.

## 0.599.0 - 2026-10-04

### Fixed

- **A card's answer is written down however many cards were open.** Locust keeps the last 64 cards it raised so it can write each answer into the conversation's record. A card was kept even after it was answered, so past 64 open cards the oldest unanswered one was forgotten, and its answer, when it came, went unrecorded and "don't ask again" on it said the card was no longer waiting. An answered card now leaves the list the moment its answer is recorded, so the 64 are all cards still waiting.
- **The saved record names the third kind of call it cannot show.** Its "Not recorded" sentence listed two: a card no one answered because the run ended, and a call made in a mode that asks nothing. There is a third: a call covered by an earlier "Always" on the same run, which raises no card. The sentence now says so.

## 0.598.0 - 2026-10-04

### Fixed

- **Locust itself, not only the card, refuses to remember "Always" for a command that reaches other programs.** Since 0.579 the approval card for a command that stops other programs, signs you out or shuts the computer down hides "Always allow this session" and the offer to save a rule. Behind the card, Locust still took an "Always" if one arrived by another route and would have remembered it for every later command of that run, and it would have saved a rule for the same command without looking. Now Locust decides from the same rule the card uses: such an "Always" is applied as "this once", the record of the conversation says so ("asked each time: stops every python.exe"), and a rule for it is refused with the card's own sentence. An ordinary command's "Always", a denial and a question's answers are unchanged.

## 0.597.0 - 2026-10-04

### Fixed

- **A file a teammate changed in a folder Locust does not read says so.** Locust never reads the text of a file inside a dot-folder, or a binary, or a very large file, so it cannot draw the change; until now the row for such a file said only that the runtime "did not report the change", which was true and told you nothing. When the folder itself is a dot-folder (a team working in `~/.claude`, say), that was every file. Now Locust still sees that the file changed, and the row says "changed · seen on disk" beside the teammate's own word for what it did (Write, Edit), with no diff underneath. Files Locust can read are unchanged: they carry their diff as before.

## 0.596.0 - 2026-10-04

### Fixed

- **A teammate running inside the Antigravity app shows what it changed.** Antigravity's transcript names the file an edit touched and never the change, and the route that runs inside the Antigravity app was the one route that never looked at the disk afterwards. Now it looks at the folder before the agent is told to begin and again when the run ends: a file the agent named gets its change on its own row, one it never named gets a row of its own, and when two Antigravity runs shared the folder the thread says so instead of counting the changes as either's.
- **Antigravity's rows use Claude Code's words.** A file row read `write_to_file` or `replace_file_content`; it now reads Write or Edit, the way the same work reads on the other runtimes.
- **A file outside the project folder reads by its last two names.** A row for a file edited in another checkout showed the whole absolute path and nothing else fit on the line. It now shows the last folder and the file, behind an ellipsis, with the whole path when you hold the pointer over it. Files inside the folder read as before.

## 0.595.0 - 2026-10-04

### Fixed

- **A project folder given by its Windows short name works for own branches.** Windows keeps a second, eight-character spelling of many folders (`C:\Users\RUNNER~1\...` for a user named runneradmin) and some tools hand that one out. git prints the real one, and Locust compared the two as text: such a folder was refused as "inside a repository but not its root", and its teammates' own branches were listed as none. Locust now takes the folder by its real spelling, and its trees, listings and branches agree with git.

### Improved

- **The open-source copy of Locust tests itself on every push.** The public repository on GitHub now builds Locust and runs its whole test suite on a hosted Windows machine, which is what a code-signing service asks to see. The first run found three differences between that machine and a person's: a test fixture that is not published (its tests are reported as skipped there), one test that needed more time on a slow disk, and the short-name case above. All three are handled.

## 0.594.0 - 2026-10-04

### Improved

- **A long run reads as a list of steps, with the current one open.** A group of steps opens while it is the one under way; when the run moves on to the next group, it folds back to its one-line summary ("Thought for 3s, read 2 files · 1 failed"), one press from the detail. The last group stays open when the run ends, and a group you opened or closed yourself is never touched. Until now every group that had ever been live stayed open, and a run with many steps became a wall. Nothing disappears: the summary line is still there, and one press reopens it.
- **A thought's words are not shown twice.** When a step line that is one thought is open, the line says "Thought for 3s" and the words are underneath, once.
- **Cursor's scratch files are named for what they are.** Cursor keeps large connector results in files of its own under `.cursor\projects\...\agent-tools\`; a step that read or searched one said "Searched 3a5a3562-ffd2-….txt". It now says "Cursor's saved tool result".

## 0.593.0 - 2026-10-04

### New

- **A private way to reach Locust's makers.** Settings > General has a Help block with the support address (`support@locust.lol`) and the public bug page, and what each carries. The Send feedback box now offers three ways: **Send** opens a public GitHub issue as before, **Email** opens your mail app to the support address with the report filled in (what you wrote, your Locust version and system, and a conversation only if you include it), and **Save as a file** writes the whole report, uncut, to a file you can attach to either -- an email cuts off a long conversation, the file does not. Nothing is sent anywhere until you send it.

## 0.592.0 - 2026-10-04

### Fixed

- **A teammate replying read-only is told so, and answers instead of going quiet.** A teammate in Ask who received another teammate's message would sometimes try a command, be refused by the mode, and write nothing back -- the exchange ended in silence with the answer sitting in the quoted message. The reply brief now says, when the reply runs read-only, that a command is refused and ends the run with nothing written, and to answer from the quote and its read tools.

## 0.591.0 - 2026-10-04

### Improved

- **A scheduled routine in Approve each says it will wait for you.** Fired while nobody is at the window, such a routine stops on its first card and waits there; the Routines screen now says so on the row, where the schedule is set, instead of leaving it to be found the next morning under "waiting on you". Ask and Accept edits run through.

## 0.590.0 - 2026-10-04

### New

- **Since you were away.** Come back to Locust after twenty minutes or more and Home opens with what happened while you were gone: runs that finished, runs that failed, routine times that passed while Locust was closed, and how many conversations are waiting on you now. Each row opens its record; "Got it" clears the list. The tray menu carries the same line ("Since you were away: 2 ran · 1 missed") while Locust runs in the background, and clicking it opens the window. Locust keeps the moment you were last at the window in its profile (`away.json`); nothing is sent anywhere.

## 0.589.0 - 2026-10-04

### Fixed

- **An inverted grep in a kill pipeline no longer names a match.** A pipeline that filtered process ids with `grep -v` (or `findstr /v`, or `-NotMatch`) before `xargs kill` was read as stopping every match of the inverted pattern. An inverted step names what is left out, so the reach line now looks past it to the match before it: filtering the grep itself out of a `ps` listing still says "stops every python".
- The Board's quiet Done list judges the trash by every turn of a conversation, as the columns do.

### Improved

- Two packages from the first design (`contracts`, `runtime-core`), which nothing in the app imports, moved out of the workspace into `attic/`. `pnpm check` builds and tests less; nothing you use changes.

## 0.588.0 - 2026-10-04

### Fixed

- **Connector rules a Cursor run left behind are taken back.** Since 0.584 a Cursor teammate running outside Auto is lent `Mcp(server:*)` rules in the folder's `.cursor/cli.json` and gives them back when its run ends. A run that was still going when Locust quit or crashed never gave them back, so the next run in that folder had connector access it was never given, and the folder kept a file nobody wrote on purpose. Locust now writes down what it lent (`cursor-connector-holds.json` in its profile) and, when it next starts, takes back exactly those rules: a file or folder the run created goes; anything you wrote stays.

## 0.587.0 - 2026-10-04

### New

- **Remove all saved rules at once.** Settings > Teammates > Saved approvals has a "Remove all rules" row under the list: it asks first ("Remove all 3 saved rules? Every card asks again afterwards."), and "Keep them" backs out. Until now each rule went one at a time.

### Fixed

- **Copilot's commands read as commands.** A Copilot CLI teammate's `powershell` tool was listed as a generic tool call, so its steps read "Ran a tool" instead of naming the command. `powershell`, `pwsh` and `cmd` now count as shell tools everywhere a command is shown, counted or asked about.
- **A saved record no longer claims an answer it does not hold.** Since 0.576 every card you answer is written into the conversation's record; if that write ever failed, the record still said "every card answered on this turn, with who answered it". Now the write is tried again when the turn ends, and if it still fails a note goes into the record ("The answer to a card (command, allowed) could not be written to this record: ..."): the Recorded line says how many answers are missing, and the note under "How it ended" says which.

### Improved

- Launching Locust from Copilot CLI's own folder (`.copilot`) falls back to the folder chosen last time, and picking it in the window asks first, the same as `.claude`, `.codex`, `.cursor` and `.gemini`.
- The "Start with Windows" switch's note says "Windows only for now." on another operating system, instead of explaining development builds.
- The "keep running in the background" preference is written whole or not at all, like the other settings files.
- The tray menu is rebuilt only when its words change, instead of every five seconds.

## 0.586.0 - 2026-10-04

### Fixed

- **A turn that did nothing still tells you what happened.** A run that
  failed before it said anything, such as a model name the agent did not
  recognise, or an Antigravity conversation its CLI no longer had, left the
  conversation blank beside a sidebar saying "failed"; and a warning the
  agent gave before any work, such as Copilot's "third-party MCP servers are
  disabled by your organization's policy", was never drawn when no work
  followed. Both are now one line in the conversation, said once.
- **The working line says Writing while a reply arrives, on every agent.**
  On Cursor the step's own words, such as "Running the tests", kept leading
  the line while the reply was already arriving. They come back once the
  text is complete.

### Improved

- **More spellings of stopping programs by name are named.** `tskill`, and
  PowerShell's CIM and WMI terminate calls (Get-CimInstance or Get-WmiObject
  on Win32_Process, piped to Invoke-CimMethod Terminate or to a
  ForEach-Object that calls Terminate). `shutdown /l` now says it signs you
  out of Windows, closing every program you are running; `shutdown /h`
  (hibernate) closes nothing and is not named.

## 0.585.0 - 2026-10-04

### New

- **The Board.** A new place in the sidebar, beside Conversations, Rooms and
  Routines, shows every conversation in columns by what it needs from you:
  **Needs you** (a card or a question is waiting, and the card says what
  for), **Working**, **Ready to look at** (it finished while you were
  elsewhere and you have not opened it since) and **Done**. Each card wears
  its teammate's face; pressing it opens the conversation. Across every
  agent at once. When nothing is waiting or running, the Board says so and
  lists what is done.
- The sidebar's places now sit two by two, so all four read whole.

## 0.584.0 - 2026-10-04

### Changed

- **A turn's steps show as they happen.** While a teammate works without
  saying anything, the group of steps at the end of its turn is open, each
  call appearing as it lands, the way Claude Code shows them. A ten-minute
  run used to be one line with a growing count. A group stays as you left
  it; one press folds it.

### Fixed

- **Cursor gives back the rules it added.** A Cursor run outside Auto adds
  connector rules to the folder's `.cursor/cli.json` so its connectors work.
  They stayed, so a later Ask run inherited access it was never given, and
  the folder kept a file you did not write. Each run now removes exactly the
  rules it added when it ends, leaves any that were there before or that you
  added, and deletes the file when it made it and nothing else is left.
- **A teammate named like an AI agent is said as a teammate.** A teammate
  asked to send its report "to Codex" sent it to the teammate named Codex,
  which started a run. The roster now lists such a teammate as
  "Codex (Code & Migrations; a teammate here, not the Codex program)", and
  naming a teammate after an agent shows a note in the editor.

## 0.583.0 - 2026-10-04

### Changed

- **Home is centered and fits two rows of teammates without scrolling.** At
  a window around 1200x780 with six teammates, Home ran a little taller than
  the window and opened scrolled down, cutting off the top of the cover. Now
  the page is centered. When it is short of height, the space between its
  sections tightens before anything is cut. A page that still runs over
  opens at the top.
- **Claw'd sleeps on the cover.** The locust there drew with stair-stepped
  legs and antennae, and no fur, under Plush. The sleeper on the cover is now
  Claw'd, who takes the fur. The swarm still flies past.

## 0.582.0 - 2026-10-04

### Changed

- **Choosing an agent's own folder asks first.** If you pick `.claude`,
  `.codex`, `.cursor`, `.gemini` or `.config` in your home folder, your home
  folder itself, or Windows' folder as the place your teammates work, Locust
  now says what is in it (settings, sign-in files, saved conversations) and
  offers to choose another folder. **Work here anyway** still works there.

## 0.581.0 - 2026-10-04

### Fixed

- **The working line stays while a reply arrives.** The line under a running
  turn, with its clock, disappeared for as long as the teammate's reply was
  streaming in, and came back after. It now says **Writing** with the turn's
  clock still running, the way Claude Code keeps its status line through a
  reply.

## 0.580.0 - 2026-10-04

### Fixed

- **A page a teammate makes runs in its turn again.** A new `index.html`
  sat as a folded row under the reply instead of running, in any
  conversation and after Keep brought a comparison's page into your folder.
  It now opens and runs on arrival. A changed page, and any other file,
  stays folded as before, and a comparison's columns still run each page
  once, above the column.

## 0.579.0 - 2026-10-04

### Changed

- **A command that reaches beyond its own work is asked about every time.**
  Its approval card no longer offers "Always allow this session" or "Yes,
  and don't ask again". For `taskkill /IM some.exe`, the agent's own
  "Always" would have allowed every `taskkill` for the rest of the run, from
  a card about one. Approve once and Deny are still there.

## 0.578.0 - 2026-10-04

### New

- **A command that reaches beyond its own work says so.** When a teammate
  runs something like `taskkill /IM python.exe`, the command row now says
  "stops every python.exe", and hovering says it in full: it stops every
  Python program on this computer, not only the ones the run started. If you
  are asked first, the approval card has a **Reaches** line saying the same,
  before you answer. It covers stopping programs by name (`taskkill /IM`,
  `Stop-Process -Name`, `pkill`, `killall` and piped forms), by port, every
  program of a user, and restarting or shutting down the computer, including
  inside `powershell -Command`, `cmd /c` and `bash -c`. Stopping one program
  by its number says nothing more, and neither does a command that only
  mentions one of these.
- **Locust is open source**, MIT licensed, at
  github.com/locust-lol/locust-app.

## 0.577.0 - 2026-10-04

### New

- **Plush.** A new choice in Settings > Appearance draws every teammate in
  soft faux fur instead of plastic. Where a teammate is drawn small, as in
  the sidebar, the fur is a shorter, combed pile, so it stays a clean plush
  rather than a fuzzy blur. Off unless you turn it on.

### Fixed

- **The mech's antennae** were drawn as a black scribble between them in
  the newer avatar library; each antenna is now its own piece. The swarm's
  antennae are a touch thicker, so both of them always show.

## 0.576.0 - 2026-10-04

### Improved

- **Every approval you answer is now kept with the conversation.** Locust
  recorded a call you declined, but nothing of one you allowed, or of who
  answered: you on the card, or a rule you saved. Now each card you answer
  is written down with its turn: what it asked, your answer, who gave it,
  and anything you said with it. "Save the record" lists them, so the file
  shows what you let a teammate do. Conversations from before this build
  keep saying what they could not record.

## 0.575.0 - 2026-10-04

### New

- **Save the record.** Right-click a conversation, or use the command
  palette, and choose "Save the record…" to save the whole conversation as
  one Markdown file: what was asked, what each agent said, every command
  with its output, every file change with its diff, hand-offs, and how each
  turn ended. A tick adds the raw record as JSON beside it. The file says
  only what Locust recorded, and says so where something was not recorded.
  Nothing is uploaded; the file is yours to read before you send it.
- **Start Locust when you sign in to Windows.** A switch in Settings, off
  until you turn it on. Locust then starts in the tray with no window, so
  your routines run after a restart without opening the app.
- **Keep Locust running when the window is closed.** Also in Settings and
  off by default: closing the window hides Locust to the tray, and routines
  carry on.

### Improved

- **A routine whose time passed while Locust was closed says it was
  missed,** with Run now, instead of starting by surprise when you open
  Locust.

### Fixed

- **Codex config warnings are shown.** Codex sends them with a summary
  rather than a message, and Locust dropped them.
- **Cursor's reconnects are said in words.** A run where Cursor lost its
  connection showed "Unhandled Cursor record: connection" and "retry"; it
  now says Cursor is reconnecting and then picking the turn back up.

## 0.574.0 - 2026-10-03

### Improved

- **Screen faces stay put on their teammate.** A teammate wearing a screen
  had it slide across its head as it turned, and at a wide turn one eye was
  cut off at the edge. The screen is now set flat into the front of the
  body, so it turns, tilts and narrows with it, and both eyes stay on it.
- **No mouth on a screen.** A teammate with a mouth that wears a screen now
  shows only its eyes; the screen's eyes say how it is.
- **Eyes on a screen have a little weight.** They trail the body by a touch
  when it hops or turns, and settle again, instead of moving as one piece
  with it.

## 0.573.0 - 2026-10-03

### Improved

- **Switching between long conversations no longer freezes the window.**
  Opening a long conversation built every message it ever had, and in a
  conversation with 1,500 messages that froze Locust for most of a second
  on every click. A conversation now opens on its newest turns, and the older
  ones appear as you scroll up to them, or when you press "Show earlier
  turns" at the top. Clicking into that conversation took 0.8 seconds and
  now takes about 0.2.
- **Teammate faces in a conversation draw together.** Each face beside a
  turn measured itself on its own, which made the window redo its layout once
  per face. They now measure in one pass.

## 0.572.0 - 2026-10-03

### Fixed

- **A teammate cut off is not said to have finished.** When a teammate you
  wrote to hit its usage limit partway through, the conversation that asked
  read "Codex finished without writing back". It now reads "Codex stopped
  before writing back: its usage limit was reached", and a half-finished
  run's last progress line is no longer brought back as if it were the answer.
  A conversation resumed on another model is also told how the earlier turn
  ended.
- **Locust no longer works in an agent's own settings folder by accident.**
  Opened from Claude Code's folder (`.claude`), Codex's, Cursor's, Gemini's,
  your home folder or Windows' folder, Locust took that folder as the place
  your teammates work. A teammate in `.claude` can read Claude Code's own
  files. Locust now opens the folder you used last, or its own Locust folder,
  instead. Choosing a folder yourself still works as before.
- **Antigravity's blocked reads are named as reads.** When Antigravity was
  not allowed to open a file outside the conversation's folder, Locust said
  it "was not allowed to change files". It now says it could not read a file
  outside the folder.

## 0.571.0 - 2026-10-03

### Fixed

- **Usage on Home stays for every AI agent.** Each agent's last usage reading
  came only from the twenty newest conversations, so an evening of Codex and
  Antigravity work made Claude Code's reading disappear. Each agent's newest
  reading is now kept.
- **A long Antigravity turn reads as one line.** Antigravity thinks before
  every step and says nothing about it, and each of those thoughts started a
  new line: one two-minute turn showed 29 lines of "Thought for 4s, listed a
  folder". A thought starts a new line only when it says something.
- **A note is said once a turn.** Copilot's "reported a change to its
  background tasks" could appear eight times above one answer; it now appears
  once, with the turn's other notes.
- **After you keep one model in a comparison, the others no longer show its
  page.** A column you did not keep showed the kept model's page under its
  own name; it now says its copy was removed.
- **Switching a conversation to another agent says how the last turn ended.**
  When Codex stopped on its usage limit, the next agent was told Codex had
  "finished its last turn"; it is now told the turn ended on the limit.
- **The Stopped card adds up.** Its plan line counts finished, cut-off and
  never-started steps from the plan itself, the workspace folder is no longer
  listed as something it finished, and an OpenCode command that may still
  have been running is mentioned once.

### Changed

- **Comparisons:** the box's model chip lists every compared model when you
  hover it, and "Ask a judge" uses the same dropdown style as the rest of
  Locust.

## 0.570.0 - 2026-10-03

### Fixed

- **A finished message stops showing the typing cursor.** Antigravity never
  marks a message as finished, so the blinking `_` stayed at the end of every
  message it had already sent, through all the steps after it. A message is
  now finished once anything comes after it, for every AI agent.
- **Antigravity's check-back timer reads as a wait.** The steps line said
  "used schedule"; it now says it waited for a command.
- **The queued-message hint names the agent.** In a conversation without a
  teammate it said "it goes to This teammate when this finishes"; it now
  names the AI agent, such as Antigravity.
- **Removing pets:** the "Removed 3 pets" message clears once you pick a pet,
  instead of staying beside the new count, and a ticked pet's mark is easier
  to see.

## 0.569.0 - 2026-10-03

### New

- **Remove several pets at once.** In a teammate's look, **Remove some…**
  turns the Pets row into ticks; **Remove N** takes the ticked pets out of the
  picker and off this computer. A pet a teammate wears keeps its files, so
  their face stays until they get another look. **Show removed** lists them
  again, and picking one brings it back.
- **A message waiting for its turn survives closing Locust.** A message
  queued behind a running teammate ("Sends when Ash finishes") is kept with
  its conversation. When you open Locust again it is offered back with Edit,
  Discard and Send. It is never sent on its own after a relaunch.
- **Start Claude Code sessions on this computer from claude.ai.** Settings >
  AI agents has a switch, **Let me start sessions on this computer from
  claude.ai**, that runs Claude Code's Remote Control in your project folder
  while Locust is open and shows what Claude Code prints. It needs your
  claude.ai sign-in; if Claude Code asks you to trust the folder, Locust opens
  Claude Code in a terminal for you to answer, and never answers for you. It
  turns off when you quit Locust.

### Improved

- **The line under a running teammate says what it is doing.** Like Claude
  Code's status line, it now leads with the step under way: "Reading
  notes.txt", "Running npm test", "Editing summary.txt", or the plan's
  current step, with the turn's clock beside it. It said "Working…" for most
  of a run before.

### Fixed

- **An older comparison shows every model's answer.** Opening a comparison
  from a day or more ago showed only the first column's answer; the others
  read "No answer was recorded" though their answers were saved. Every column
  is read now.
- **Home works from a comparison.** Clicking Home while a comparison was open
  did nothing when the comparison had no teammate.

## 0.568.0 - 2026-10-03

### Improved

- **Screen faces move steadily.** A teammate with a screen for a face used to
  spin a full turn every few hops while it worked, so its screen swept to the
  back of its head and its eyes clipped away, and it squashed hard on every
  landing. Its eyes also flickered into dashes every few seconds. A screen
  now keeps facing you: it still hops, leans, glances and follows your
  pointer, but it never spins, lands more softly, and its eyes only change
  when its state does. Teammates with their own eyes move as before.

### Fixed

- **A Claude cloud message Locust could not confirm is no longer called "not
  sent".** When Claude Code ended without saying whether a follow-up reached
  your cloud session, Locust said it "did not send that", which invited
  sending it again, so the session could do the work twice. Locust now says
  it could not confirm the send and asks you to look at the session on
  claude.ai first. When Claude Code says a send failed, Locust still shows
  its words.

## 0.567.0 - 2026-10-03

### New

- **Routines can ask for answers when they run.** Write `{{topic}}` in a step
  and add an input for it: a line of text, a longer text, one of a few
  choices, or a folder. Pressing **Run** asks for each answer, with defaults
  filled in, and the step goes out with your answers in place. A routine that
  runs on its own uses the defaults; when a required answer has none, it says
  so and starts nothing. A folder can only be chosen with the folder picker,
  never typed.
- **Routines travel as files.** **Export** saves a routine as a
  `.locust-routine.json` file: its steps, the inputs it asks for, who should
  take each step, and the connectors it needs. It never includes your answers,
  your history or anything about this computer. When a step names a path on
  this machine, Export offers to make it an input instead. **Import routine**
  shows the whole routine before anything is saved, says which connectors
  you have, and lets you choose the teammate who takes it. An imported routine
  arrives in Ask, with no schedule, and runs nothing until you press Run.

## 0.566.0 - 2026-10-03

### New

- **Settings > AI agents says what GitHub Copilot can do.** When Copilot runs
  a turn in Approve each, it tells Locust what it supports: continuing an
  earlier session, pictures, sound or a file's contents in a message, and
  connectors over the web. Its row now lists each as a plain yes or no, as
  Copilot's own answer and from when. Nothing here changes what Locust lets
  Copilot do: it is still offered no files and no terminal of its own.
- **Settings > Connectors lists your connectors and how each is.** Each one
  says Connected, Needs sign-in, Not working, Did not answer or Unknown, with
  what to do about it ("Run /mcp in Claude Code and sign in to Notion") and
  when it last worked. **Copy report** puts names, states, timings and hosts
  on the clipboard for someone helping -- never a command, a key or a full
  address. Locust never calls a connector's tools to test it.

### Improved

- **A connector check that takes too long says so.** When Claude Code's
  connector check did not answer in 20 seconds, Locust quietly kept the last
  list; Settings now says the check did not answer, while your teammates keep
  using the connectors that last worked.
- **Switching a conversation to another AI tells it what was actually done.**
  The summary the new one starts from listed Antigravity's work as
  `run_command: run_command` and `view_file: view_file`, forty times. Each
  action is now named by the command it ran or the file it read or wrote.
  Past the latest forty, the summary says how many earlier ones it leaves
  out, rather than claiming they were never recorded.

### Fixed

- **A conversation you hand to another teammate takes the message box with
  it.** After **Assign to** on the conversation you were looking at, your
  reply still went to the teammate who had it before. They no longer owned
  it, so it started them a new conversation instead of continuing this one.
  The box now sends to whoever you assigned it to. If a reply ever would start
  a new conversation, the box says so before you send.

## 0.565.0 - 2026-10-03

### Improved

- **A tidier Pets row.** The small arrow on every pet not yet on this computer
  is gone -- on a new computer that was an arrow on all 21. One line under the
  pets says it instead: each downloads from openpets.dev the first time it is
  picked.
- **Dark pets show in the picker too.** A pet drawn in black, like Reaper, now
  has a faint light edge in its tile, as it already had on a teammate's face.
- **A bigger face beside the name** in a new or edited teammate, so the look
  you pick is easier to see.

## 0.564.0 - 2026-10-03

### Improved

- **Pets: a short list that fits Locust, in place of the gallery.** The open
  gallery mixed every style of community art and did not sit well beside the
  bots, so it is gone: no search, no Featured or Originals. The Pets row in a
  teammate's look now offers 21 pets -- little robots, terminals and
  screen-faced gadgets -- each shown by its picture. One that is not on this
  computer yet carries a small arrow; click it and that one pet downloads from
  openpets.dev and goes on the teammate. A teammate already wearing a pet keeps
  it.
- **Pets move more smoothly.** Each drawing now melts into the next instead of
  snapping to it, and a pet drawn mostly in black has a faint light edge so it
  no longer disappears into the dark sidebar.
- **A teammate that asked you something waits on you.** When a teammate ends
  its turn on a question, its face now waits inside the amber ring, as it does
  for a paused run, instead of resting while the title bar says it needs you.
- **The AI agent's badge on a face is smaller** and sits further off the body,
  so a small face in the sidebar reads as a face.
- **Claude cloud sessions can be watched.** With a session's row open,
  **Watch** reads it again every minute and a half and says when it last
  updated; it stops on its own when two reads bring nothing new, after half an
  hour, or when you close the row.
- **Continue a Claude cloud session in your terminal.** A cloud row offers a
  terminal copy of the session, in its own checkout beside your folder, and
  says plainly that Locust cannot see what happens there. (Checked by tests;
  no terminal was opened while building it.)
- **Claude cloud can start in your organization's own environment.** Starting
  a cloud task offers an optional environment ID, checked before anything
  runs and remembered for that folder.

### Fixed

- **A message sent to a Claude cloud session is confirmed by Claude Code
  itself.** Locust used to look for "Sent to cloud session" in the terminal's
  words; it now reads Claude Code's own answer, shows its error word for word
  when a session is archived or gone, and never reports a sent message as
  unsent.
- **A Claude cloud session that no longer exists says so.** Reading one that
  was archived or deleted said Claude Code "did not save its conversation this
  time"; it now says the session is not found in Claude's cloud, and removes
  the empty copy Claude Code had started for it.

## 0.563.0 - 2026-10-03
<!-- big -->

### Added

- **A teammate can be a pet.** Under the bots in a teammate's look there is
  now a row of pets. OpenPets' Hoodie Cat comes with Locust, and any pet you
  made in Codex shows up there too. **Browse the gallery** opens the pets on
  openpets.dev in place, Featured or Originals, with a search. Click one and
  that pet is downloaded to this computer and put on the teammate; nothing is
  downloaded before you click, and each pet stays its maker's -- Locust ships
  none of them. Picking a pet for a teammate with no name yet names it after
  the pet. A pet is only the face: the teammate is the same in every other
  way, its model, conversations, rooms and routines.
- **A pet shows what its teammate is doing**, in its own drawings. It reads
  while it thinks, gets busy while it works, waves when a message arrives,
  waits with you inside the amber ring, jumps when it is done and slumps when
  it is stuck. At rest it keeps still, as a bot does. A pet that has gone
  missing shows the teammate's bot instead, and the look says why.

### Fixed

- **No teammate looks stuck while Locust starts.** For the first seconds after
  every launch, before Locust had found which AI agents are on the computer,
  every teammate counted as stuck, and a teammate with a screen showed red
  crosses for eyes. Until Locust knows, a teammate now simply rests.

## 0.562.0 - 2026-10-03

### Added

- **Each teammate's face is your choice: Eyes, a Mouth, or a Screen.** Pick it
  in the teammate's look. Until you pick, a screen goes on the shapes it suits
  -- the droid, the mech, Prompt, the critter, and the square, pill, hexagon,
  pebble, circle, ghost and cat -- and the rounder, grown shapes keep their
  own eyes. Settings > Appearance > Terminal faces still turns every screen
  off at once; Prompt keeps its screen, since that is its face.
- **The title screen's three each have a part.** Prompt, the Codex nod, types
  in bursts, finishes with a hop, looks over at the ghost, waits on you in
  amber, and gets back to it. The ghost thinks, has an idea, and looks across.
  The locust sleeps, eyes shut and z's drifting up, until your pointer comes
  near; then it watches you, and a click makes its day.

### Changed

- **A screen glows one terminal colour, and colour means something.** Every
  screen's eyes glow the same cyan; they turn green when a teammate is done,
  amber while it waits on you (the colour of its ring), and red when it is
  stuck. A teammate's own colour stays in its body and in the tint of its
  screen.
- **Code eyes belong to the screen.** A teammate without a screen keeps its
  own eyes, which blink, glance and laugh, and still shows what it is doing
  with its ring, its dot and its hop.
- **Eyes blink when they change** instead of jumping from one look to the
  next.

### Fixed

- **Eyes no longer run off the edge of a screen.** On a smaller screen, a
  thinking or finished teammate's eyes could poke through its top, and a
  teammate turning its head to follow your pointer could lose an eye off the
  side. Every eye now stays inside its screen, at every size, glance and
  moment.
- **A teammate that hops when it finishes lands facing you.** On a busy
  computer the hop's spin could come back mid-air and leave it turned away,
  facing backwards.

## 0.561.0 - 2026-10-03

### Added

- **Terminal faces.** Every teammate now wears a screen for a face, the way
  Prompt does: a dark screen in its own colour, sized to its own shape, with
  its eyes lit on it as code that says what it is doing. A bot with a mouth
  has its mouth lit too. It is on by default; turn it off in Settings >
  Appearance > Terminal faces and every teammate goes back to its own eyes.
  The setting shows three bots wearing your choice beside it, and it is kept
  across restarts.

### Changed

- **A teammate at work or in thought looks alert, not flat.** Working, its
  prompt now has a solid block cursor that types in bursts and blinks.
  Thinking, its eyes are round and wide open, looking up and about, then
  bouncing in turn like a reply being typed. No more underscores and dashes.
- **Teammates are drawn bigger where you watch them.** The thread's live
  "Thinking..." row, the conversation header, the Team screen's cards, the
  sidebar's faces and rail, a room's answers, and the faces beside names in
  lists and pickers are all larger, each sized for what it is doing there,
  and every row they sit in was measured to still fit.
- **A screen's eyes stay on the screen.** When a teammate turns its head or
  glances aside, its lit eyes now move within its screen instead of sliding
  off the edge.

## 0.560.0 - 2026-10-03

### Changed

- **A teammate's code eyes are drawn, not typed, and they move.** In 0.559
  the eyes were letters set in a font, and they sat still. Now each is a few
  thick, round strokes, as heavy as the eye it replaces, and each state has
  its own little loop: at work the cursor types forward and then blinks,
  thinking dashes drift and now and then pull in to dots, a finished
  teammate's `^ ^` bobs, and a stuck one's `x x` shudders. They still blink,
  turn and glance with the head. A teammate that keeps still keeps its eyes
  still too, as does everyone with reduced motion on.
- **Prompt has a screen for a face.** The Prompt bot's face is now a dark
  screen in its own colour, curved round its head, with its eyes lit on it in
  a bright tint of the same colour. At rest it shows two lit bars that blink
  and glance. Its lit eyes read even at the sidebar's small sizes.

## 0.559.0 - 2026-10-02

### Added

- **A teammate's eyes say what it is doing.** Every teammate's eyes now turn
  into code while it works: `>_` while it works, `- -` while it thinks, `^ ^`
  when it is done, `x x` when it is stuck. They still turn, glance and blink
  with the head. Talking, listening, idle and waiting on you keep the bot's own
  eyes.
- **Two new bots to pick: Critter and Prompt.** A blocky critter on four
  short legs, and a soft terminal window with its three dots, drawn in the
  same plastic and your teammate's colour as every other bot. Pick them in the
  look picker; no existing teammate's face changes.

## 0.558.0 - 2026-10-02

### Added

- **Read a Claude cloud session in Locust.** A session's row now has "Show
  what it did". Claude Code brings the session in out of sight, in a worktree
  of its own, so your folder is not touched. The row then shows the
  conversation (each task and its answer) and the session's change as a diff.
  "Apply to this folder" brings the change in, not committed, and "Check
  again" reads it afresh. No model is called to read it.

### Fixed

- **Bring it home counts untracked files.** Claude Code refuses a folder with
  untracked files, and Locust now says so before opening a window that would
  only refuse.

## 0.557.0 - 2026-10-02

### Fixed

- **A task for Claude's cloud runs on the model you picked.** Claude Code was
  given no model, so every cloud session ran on your account's default
  (Opus), whatever the box said. It now gets the box's Claude model and its
  effort.

## 0.556.0 - 2026-10-02

### Changed

- **Claude's cloud starts without a window, and Locust knows the session.**
  A task sent to Claude's cloud starts out of sight on Windows, and its row
  shows the session's title and where it starts from. "See it on claude.ai"
  opens that session, a follow-up can be sent to it from the row, and "Bring
  it home" brings that session home by name instead of asking you to pick
  from a list. Claude Code's window still opens when it needs to ask whether
  you trust the folder. Claude Code does not let Locust read a cloud
  session's replies, so the row says to read them on claude.ai or in the
  Claude app.
- **Tables, lists and headings format as they arrive.** A long table used
  to stay as raw pipes until the whole of it had been written; each finished
  row, item and heading now takes its shape the moment its line is done.

### Fixed

- **Bringing a session home into a folder with changes says so first,**
  instead of opening a window that only refuses.

## 0.555.0 - 2026-10-02

### Changed

- **Compare on Auto or Edit works in any folder.** A folder too big to give
  each model its own copy is no longer refused: they all work in the folder
  itself, and the bar says so. A git project still gives each model its own
  copy, and a plain folder now does up to 20,000 files and 1 GB, leaving out
  single files over 20 MB rather than refusing.
- **On Auto, the bar says a model can still change files outside its copy**
  if it is asked to.

### Fixed

- **A file a compared model wrote opens from its column.** Its chip looked in
  your folder rather than the model's copy, and said the file was not there.

## 0.554.0 - 2026-10-02

### Changed

- **A judge's view reads like every other reply.** While it reads the
  answers you see what it is doing -- the files it opens, its words as they
  come -- in place of a bare "Reading the answers".
- **The answer the judge would keep says "Judge's pick"** in its column's
  head. It is still the judge's view: you keep the answer.
- **Every model you can use is offered as a judge,** listed under its agent.
  Only sixteen fitted before, and the free OpenCode models took most of them.
- **A judge is never one of the models it judges** when you open a
  comparison right after Locust starts. The menu could stay on a compared
  model it showed before your agents were found.

## 0.553.0 - 2026-10-02

### Added

- **A web page in a reply runs right there.** When a model answers with a
  whole page in a code block -- a game, a landing page -- it runs on a stage
  in the reply, in a conversation and in each column of a comparison, with
  its code one tab away.
- **Antigravity has Sign in again,** like the other agents. It opens Antigravity,
  where /logout and /login switch accounts.

### Fixed

- **Ask a judge works when the answers are long.** Long answers made the
  judge's message too long to send, whichever model judged; it now reads
  them from files.
- **Compare no longer says a new folder is too big to copy.** It kept the
  answer for the folder you were in before.
- **A compared model's effort is a small meter beside its name,** so the
  names fit.

## 0.552.0 - 2026-10-02

### Fixed

- **Each chat keeps its own model, mode, effort and chat mode.** Switching
  chats carried the last chat's Plan, Ask or Compare into the next one, and a
  reopened conversation showed its teammate's model instead of the one it ran
  on. Now each conversation comes back as you left it.
- **Trying another model in a chat no longer changes the teammate's model.**
  A new chat with a teammate starts on the model in their profile; change it
  there (Edit teammate) to change it for good.
- **A new chat with no teammate starts on the last model you picked there,**
  after a restart too.
- **OpenCode's free models come back by themselves after a slow launch.**
  When its model list came back empty, the picker offered only Account
  Default for ten minutes; it now asks again until the list arrives.

## 0.551.0 - 2026-10-02

### Added

- **Several conversations with the same teammate at once.** Start another
  conversation with a teammate who is busy in one, and it runs straight away,
  beside the first. Only a reply to a turn that is still running waits for
  it. Up to the same number of runs at once as before.

### Fixed

- **A message you have not sent stays with its conversation.** It followed
  you into every other chat.
- **Opening a conversation with no teammate addresses no teammate.** The box
  kept whoever was picked before, so a message there went to that teammate,
  on their model, queued behind their other conversation.
- **An Antigravity command refused before it explained itself still says Did
  not run.** A plan Antigravity writes into its own folder is never counted
  as refused.
- **When a run fails after OpenCode's retries, the provider's own reason
  stays,** without the line saying it is trying again.

## 0.550.0 - 2026-10-02

### Fixed

- **A conversation with no teammate keeps its model's logo when you open it.**
  Opening it put the empty box back.
- **A command Antigravity was not allowed to run says "Did not run",**
  everywhere. Antigravity sometimes reports a refused command as finished and
  says it was refused only at the end; the step, its footer and Activity said
  "Ran git status".
- **Compare starts on the effort your conversation is on.** A conversation on
  Low opened Compare with its model on High.
- **"Trying again on its own" goes once the run has failed.** It stayed on a
  failed Compare column after OpenCode's retries had ended.
- **A group of reads says "Read 3 files",** in words, as Activity does, where
  it said "read 3".

### Changed

- **Cursor in Ask or Plan no longer lets every connector tool run.** Locust
  adds Cursor's allow rules for your connectors only in Accept edits; in Ask
  and Plan a connector tool runs only if you allowed it in Cursor yourself.

## 0.549.0 - 2026-10-02

### Fixed

- **A long Codex conversation can be followed up again.** Carrying on a Codex
  conversation asked Codex for the whole thread back in one message; a long
  one (measured: 9.3 MB, a 21-minute turn with screenshots) was over Locust's
  limit, so every follow-up failed with "A message exceeded the line size
  limit". Locust now asks only for what it uses (0.02 MB on the same thread).
- **That failure no longer shows an unrelated Codex warning as its cause,**
  in terminal colour codes. Colour codes are also cleaned from a runtime's
  last word on any failure card.

## 0.548.0 - 2026-10-02

### Fixed

- **A review by an Antigravity teammate now only reads.** "Ask for a review"
  runs the reviewer in Ask for that run, so it cannot change your files. An
  Antigravity reviewer was left in its own mode because Antigravity's app
  could not be held to Ask; through Antigravity CLI it can, so it is now.

## 0.547.0 - 2026-10-02

### Changed

- **A conversation with no teammate shows its model's logo** in the sidebar
  where a teammate's face goes, instead of an empty box. A model of your own
  has no logo, so it keeps the box.

## 0.546.0 - 2026-10-02

### Fixed

- **Sign in again on Codex says plainly that it signs Codex out.** Codex's own
  sign-in removes the current one the moment it starts, before you have
  answered in the browser (measured), so closing that window unfinished left
  Codex signed out. The warning now says so for Codex instead of "may".

## 0.545.0 - 2026-10-02

### New

- **Copy, as Claude Code has it.** Hover a teammate's finished reply and Copy
  takes its text, Markdown and all. A file a teammate hands over has Copy
  beside Save a copy, for its text: quicker than saving it and adding it from
  Downloads.

### Changed

- **In Ask and Plan, a Claude teammate asks before each connector call**, as
  Claude Code itself does in its plan mode (measured). Connectors it was given
  were used without asking in every mode until now. Edit and Auto are as they
  were.

### Fixed

- **Sign in again says first that it may sign you out** until you finish in
  the window, and Cancel opens nothing. A tester closed Codex's sign-in
  unanswered and Codex stayed signed out.
- **A sign-in window that closes puts its row back** at once; it kept saying
  "Finish in the window that opened."
- **Claude's cloud says when a folder is not on GitHub**: it works on a copy
  of a GitHub repository, so from any other folder it starts with no project.
  It offers your folders that are, and no longer sends you to a window that
  has already closed: the session is on claude.ai and in the Claude app.
- **Activity says "Did not run git status"** for a command the mode refused,
  not "Ran", and shows a runtime's note in its own words.
- **Compare keeps the effort you chose** for the model it starts on.
- **Every runtime's file read says Read**, as Claude's does, not `view_file`
  or `read`; searches, listings and fetches likewise.

## 0.544.0 - 2026-10-02

### Fixed

- **A file that is a link out of the folder is not opened.** The viewer
  checked a file's path, so a link inside the project that led elsewhere
  showed what it led to. It now checks where the file really is, and a
  project that is itself reached through a junction still opens its own files.
- **A window that cannot be drawn says so.** If drawing it ever fails, the
  window shows what happened and a button to reload it, instead of going blank.
- **A Claude subagent's own tools stay inside its row.** Their results were
  listed in Activity as calls named only "tool".
- **OpenCode's stop note is true.** Stopped at a command its mode does not
  allow, it said "Nothing was changed" even when it had written files before
  that. It now says what it did before still stands.
- **More secrets are scrubbed** from what a run records: private key blocks,
  bare JWTs, Google API keys and Hugging Face tokens.
- **A Mac update without a stated checksum is not installed.**

## 0.543.0 - 2026-10-02

### New

- **Antigravity joins comparisons** when it runs through Antigravity CLI.
  Through its app alone it answered only in the folder Antigravity had open,
  so it could not.
- **Open an Antigravity conversation in a terminal**: Antigravity CLI picks
  it up with `agy --conversation`, in the teammate's folder.
- **Sign in again, from Settings.** Every signed-in AI agent's row has it:
  a window opens running that agent's own sign-in (`claude auth login`,
  `codex login`, `cursor-agent login`, `muse login`, or Copilot's `/login`),
  where you can sign in as this account or another.

### Improved

- **Less work while a teammate is working.** The orb beside "working" was
  drawn at every frame the screen shows (144 a second on a gaming laptop),
  the largest single cost in the window while replies streamed. It
  is now drawn at most 30 times a second, like the teammates' faces, and
  moves at the same speed. Measured with three replies streaming at once:
  about 40% less script time. Also, a reply's finished part is parsed again
  only when more of it finishes.

## 0.542.0 - 2026-10-02

### New

- **Antigravity's thinking shows as "Thought for 4s"**, as Cursor's does.
  Antigravity CLI sends no thinking text, only how long a step took and how
  much of it was thinking, so the line has nothing to open, and a step that
  did no thinking draws no line.

### Fixed

- **Antigravity's background-command checks read "Checked on a command".**
  0.541 called them "Updated the plan"; measured, `manage_task` is a check on
  a command Antigravity sent to the background.
- **Antigravity's web searches name what they searched for.**

## 0.541.0 - 2026-10-02

### Fixed

- **A conversation that began in the Antigravity app carries on through
  Antigravity CLI.** A reply to it failed with "trajectory not found",
  because the CLI was handed the app's own conversation. It now starts the
  CLI fresh with the conversation so far, as a switch of model does.
- **A command Antigravity was not allowed to run reads "refused".** In Edit,
  its refused `git status` read "failed" and "it exited non-zero", though it
  never ran. And since Antigravity ends its turn there, the note says it
  stopped.
- **Antigravity's plan updates read "Updated the plan"**, not `manage_task`.
- **No "Experimental" tag on Antigravity** when it runs through Antigravity
  CLI. Through the app alone, it keeps the tag.

## 0.540.0 - 2026-10-02

### New

- **Antigravity runs through Antigravity CLI.** Google moved personal
  accounts, AI Pro included, from Gemini CLI to Antigravity CLI (`agy`). With
  it installed, Antigravity works like every other AI agent here: each step
  shows as it happens, the reply arrives whole, the run ends when it answers,
  and a follow-up carries the conversation on. It works in any folder, not
  only one the Antigravity app has open, and in every mode: Ask, Plan, Edit
  and Auto (only Auto lets it run commands). Rooms and tags reach it too.
- **Every model Antigravity offers**, not three tiers: Gemini 3.8 Flash,
  Gemini 3.1 Pro, Claude Sonnet and Opus, and more, with Low, Medium and High
  as the effort beside the model rather than as models of their own.

Without the CLI, Antigravity runs through its app as before, in Edit only.
Settings > AI agents links to the one-line installer.

## 0.539.0 - 2026-10-02

### Fixed

- An answer from a comparison you have not kept yet says so above the
  conversation, with **Open the comparison**, however you opened it. From
  Conversations there was no way back to it.
- After a follow-up, a comparison says why no other model can join: it would
  be asked the first question and miss the turns since. The choice used to
  disappear with nothing said.
- Your record in a comparison says **Typical tokens**, not "Typical cost",
  since what it shows is tokens in and out.

## 0.538.0 - 2026-10-02

### New

- **Claude's cloud, from the Cloud choice.** With a Claude model picked,
  Cloud in the chat-type menu sends your task to a Claude Code cloud session:
  Claude Code opens in a window of its own in this folder, with the task
  already given, and the work runs on Anthropic's machines. The first time in
  a folder, Claude asks whether you trust it; answer in that window. Each
  task is listed beside the conversation, with **See it on claude.ai** and
  **Bring it home**, which opens Claude Code here with its list of cloud
  sessions to pick from. With a Codex model, Cloud goes to Codex Cloud, as
  before.

### Improved

- A routine says it runs on the model its teammate last worked on. "The
  model at the time" was easy to read as whatever the message box showed.

## 0.537.0 - 2026-10-02

### Fixed

- **An Antigravity run that reads its subagents' notes finishes when it
  answers.** Reading a file that quoted another task's "running as a
  background task" line was taken as background work of its own, which never
  ended, so the turn kept saying the agent was working and its final answer
  never showed as the reply. Only a result's own task counts now.

## 0.536.0 - 2026-10-02

### Fixed

- **Stopping an OpenCode run no longer promises that nothing ran.** OpenCode
  reports a tool only once it has finished, so a command it had just started
  may not show. The Stop card now says so, never claims no tool call was
  open, and asks you to look at the folder before you send the message again. Other runtimes report a tool when it
  starts, and their card is unchanged.
- A routine whose check still failed when its fixes ran out says so on its
  card under Routines ("Last run: ..."). Before, the card read like a normal
  finished run.
- A routine whose changes wait in its copy now says, on the card itself,
  that it runs again once you Keep or Discard them.
- Ctrl+K says Conversations, not Missions.
- A reopened conversation that switched model says "The person chose to
  leave out its last reply", not "leave out: summary".

## 0.535.0 - 2026-10-02

### Fixed

- A new routine for a teammate that has never had a conversation now asks
  what a run may do, like any other, and saves the choice it shows. Before,
  the choice was missing, the dialog said changes would land straight away,
  and the routine was saved to only read.
- In a long conversation, replies line up with the typing box again. The
  scrollbar had pushed them 5px to the left; on a narrow window their right
  edge stopped 10px short.

## 0.534.0 - 2026-10-02

### New

- **A routine can keep going until your check passes.** A routine that may
  change files can now say what happens when its steps are done: stop, or
  run this folder's check (the command you set in Settings > Project
  folder, such as `npm test`) and, while it fails, ask the teammate to fix
  what the check printed, up to the number of fixes you choose. When the
  check passes it says so; when the fixes run out it stops, says what still
  fails, and does not count the run as done. Each fix is one more turn.
  In a copy, the passing result waits for you to Keep as usual.

### Fixed

- A routine that works in a copy lists the files it changed by their names
  in your folder, not by the copy's long path.

## 0.533.0 - 2026-10-01

### New

- **A routine can work in a copy, and nothing lands until you keep it.**
  A routine that may change files now asks where it works: in the folder,
  or "in a copy, you keep". In a copy, each run works on a copy of your
  folder. When it finishes, its card under Routines says which files it
  changed: Keep writes them into the folder, Discard throws them away, and
  Open the copy lets you look first. Keep refuses, and says so, if you have
  changed the same files since. While changes wait, the routine does not run
  again.

## 0.532.0 - 2026-10-01

### Fixed

- **A comparison never saves to a teammate's own branch.** For a teammate
  with Own branch on, a Compare column, a judge or a side question could
  save a turn to that teammate's branch when it finished, under the
  comparison's question, taking along anything a conversation with the same
  teammate was in the middle of changing there. Only the teammate's own
  conversations save to its branch now.

## 0.531.0 - 2026-10-01

### Fixed

- **A Codex turn on its own branch is saved as completed.** With Own branch
  on, each turn is saved to the teammate's branch with how it ended. On
  Codex every turn was saved as failed, even when it finished; it now says
  completed, or stopped when you stopped it.

## 0.530.0 - 2026-10-01

### New

- **New routine.** The Routines screen has a New routine button: write the
  steps yourself and pick when it runs. Saving a finished conversation as a
  routine still works too.
- **Choose what a routine may do.** A routine now asks whether a run may
  only read or may change files, and everything it says follows that
  choice. A routine that starts when a file arrives used to say it changed
  nothing even when it could.

### Improved

- **A teammate in Ask can read Word and PowerPoint files.** Locust keeps a
  text copy of the words in each one, so a teammate that may only read can
  still answer from your documents. The copies live in Locust's own folder
  and never change the documents.
- **A document's change reads as the document.** "What this turn changed"
  on a Markdown document shows each changed passage with the old words
  struck and the new ones marked, instead of a code diff. The lines are one
  click away.
- **A new teammate starts as Research & Briefs**, ready to read whatever is
  in your folder, rather than as a programmer.

## 0.529.0 - 2026-10-01

### Improved

- **A reply lines up with the box you type in.** A teammate's words now
  start where your own typing starts in the box below and run to where it
  stops, with the teammate's face in the margin beside them, the way Claude
  lays out a conversation. Before, a reply stopped well short of the box's
  right edge and started a little right of it. On a very wide window the
  conversation and the box stay a readable width.

## 0.528.0 - 2026-10-01

### Fixed

- **A model has one name.** The model list showed Codex's "GPT-6-Luna" while
  the chip beside the box, and a Compare column, said "GPT-6 Luna". The chip
  now spells a model the way its AI agent does.

## 0.527.0 - 2026-10-01

### New

- **Leave part of a handoff out.** When your next message goes to another
  AI agent, the line under the box says what it carries. The earlier
  messages, the steps it finished and its last reply each have a small ×
  now: press it when one is stale or would mislead, and it is not sent.
  Put back undoes it. The task, your own words and any steps that never
  reported back always go. After the switch, the divider says what you
  left out, and still does when the conversation is opened again.

## 0.526.0 - 2026-10-01

### Improved

- **Missions are called conversations now.** The sidebar and the screen
  that lists them say Conversations, the send button is Send, and Stop
  stops "the running reply". A teammate's card counts its runs. Settings
  says every conversation is kept on this machine, in a record that is only
  ever added to, and clean-up counts the turns it would delete.
- **The sidebar shows all three places whole.** Conversations, Rooms and
  Routines each take the width of their word, so none loses its icon.

## 0.525.0 - 2026-10-01

### Fixed

- **A teammate's card no longer says "not run yet" beside its last run.**
  On the Team screen a teammate without a model of its own read "not run
  yet" right next to "last run 5 minutes ago"; it now says it runs on the
  model you pick.
- **Plainer words in a few more places.** The conversation list says "all
  readable" instead of "ledger readable"; Swarm and Auto mode in Settings
  speak of models and the project folder rather than routes and the
  workspace; Rooms says each teammate answers on their own AI agent and
  model.

## 0.524.0 - 2026-10-01

### Fixed

- **"Ask about this" no longer covers what you are reading.** It used to sit
  over the line above the words you selected; it now sits just after them,
  on their own line.
- **A file that cannot be opened is named.** The message says "Could not
  open report.md" rather than "that file", with the reason under it.

## 0.523.0 - 2026-10-01

### New

- **Ask another model the same question in Compare.** A comparison with one
  question and room for a third column offers "Ask another model": pick one
  that is not in it yet, and it is asked exactly what the others were, in a
  new column. What you kept stays kept, and your record counts the new one.

## 0.522.0 - 2026-10-01

### New

- **A routine can run when a new file arrives.** Choose "On a new file"
  under Runs on its own and name a folder inside your project (inbox to
  begin with). A file that lands there starts the routine once it has
  finished copying, and the teammate is told which file it is for; files
  already there start nothing, and it runs at most six times an hour. As
  every routine does, it reads and changes nothing.

### Changed

- The routine choices read a little shorter: "When I press Run" and
  "Daily".

## 0.521.0 - 2026-10-01

### New

- **Approval cards can remember your answer.** When a teammate asks before
  running a command, changing a file or using a connector, "Yes, and don't
  ask again" saves a rule for exactly that, for that teammate in that
  folder, and beside Deny, "Never allow this" saves the opposite. The card
  says the rule in a sentence before you press it, the conversation says
  when a rule answered for you, and Settings > Teammates > Saved approvals
  lists every rule with how often it was used, to remove any of them.

## 0.520.0 - 2026-10-01

### New

- **Ask a judge in Compare.** Once two answers are in, you can ask another
  model for its view: it reads the answers without knowing which model wrote
  which, against what you say a good answer does if you want to say, and
  tells you what each did well, what each missed, and which it would keep.
  It is a view, not a decision: you still keep the answer yourself.

### Fixed

- **In a comparison, the line above the box no longer says your message
  goes to one AI agent with a summary.** A question asked there goes to
  every model in the comparison.

## 0.519.0 - 2026-10-01

### New

- **Compare keeps your record as a table.** Once you keep an answer, the
  comparison shows every model you have compared: how often you kept it,
  and how long its answers typically took and what they typically cost, so
  the strong-and-cheap one stands out.

### Improved

- **"Own branch" is now "Own copy".** The switch on a teammate's card says
  what it does: the teammate works in its own copy of the folder. The
  branch it works on is still named under it.
- **A switch to another AI agent still says what it left out after you
  reopen the conversation.** The line under the divider used to disappear
  once the conversation was closed, and it now names the parts in plain
  words ("its last reply", "the earlier messages").

## 0.518.0 - 2026-10-01

### Improved

- **Before a reply goes to another AI agent, Locust says what it takes
  along.** When your next message will continue a conversation on a
  different AI agent, the line above the box now says what the summary
  carries (the task, the earlier messages, the steps it finished, its last
  reply) and, as you type a long reply, what is left out to fit. It used to
  say this only after the switch.
- **Plainer words.** Settings, the message box, Home and the teammate form
  now say "AI agent" where they said "runtime", "model" where they said
  "route", and say what a mode or a hand-over does instead of "sandbox" and
  "checkpoint". The Settings page is now called AI agents, and the sidebar
  says how many are ready.

## 0.517.0 - 2026-10-01

### New

- **Word and PowerPoint files open in Locust.** A `.docx` or `.pptx` a
  teammate makes now opens in the file viewer as what it says: headings,
  paragraphs, lists and tables, and a deck slide by slide under each title.
  It is a reading view: pictures and layout are not shown, and nothing in the
  file runs.
- **Routines can run on set days, or once.** Next to "Every few hours" and
  "Daily at a time", a routine can run on the days you pick (Monday to
  Friday to begin with) or once, at a date and time. A day missed while
  Locust was closed runs once the next time it is open.

### Improved

- **An earlier version of a document reads as the document.** In the file
  viewer, pressing a turn's number shows the file as it was after that turn,
  not a list of changed lines; what that turn changed is one press away.
  When the record cannot rebuild a version exactly, it shows the change and
  says why.
- **"Use a free model" picks one that works here.** It, and Compare's second
  free model, now prefer the free model that last answered on this machine.
  The first free model in OpenCode's list can be down for a whole day.

## 0.516.0 - 2026-10-01

### Added

- **Locust on a Mac updates itself, like on Windows.** A Mac copy in your
  Applications folder now checks for new versions, downloads them quietly,
  and offers **Restart and install**: it quits, puts the new version in place
  (the old one is put back if anything goes wrong) and opens again. The
  download is checked against the release before anything is installed, and
  the new copy opens without asking you to allow it again. To get here from
  an older Mac copy, download this version once and drag it into
  Applications; your conversations and settings stay.

### Fixed

- **A file a teammate hands over from another folder you work in now gets its
  button.** A teammate working in one folder that wrote a file in another
  (handed over as `../other-folder/report.md`) got no button, and learned to
  copy files into its own folder instead. It is now shown, from any folder
  Locust works in; anywhere else is still refused when you press it.
- **Home no longer shakes at some window sizes.** With a few teammates, a
  window around 1,200 by 770 could make Home flick between two layouts many
  times a second, most often right after removing a teammate. Home now
  settles on one layout and holds it at every size.

## 0.515.0 - 2026-10-01

### Changed

- **The chat uses more of the window.** The conversation and the message box
  now grow with the window from about 1,100px wide (and in the narrow layout
  with the sidebar folded), so a mid-size window no longer keeps a wide empty
  margin either side.
- **A thought reads one way.** A model's thinking inside a group of steps is
  shown as its words, quietly, the same as a thought on its own line, instead
  of a second "Thought for 3s" row that opened onto its title again.

### Fixed

- **Installing a CLI on a Mac.** Where npm's global folder belongs to the
  system, Install failed every time with "permission denied". It now installs
  into a folder of your own (`~/.npm-global`), which Locust already looks in.
- **A Mac says when a new Locust is out.** A Mac copy cannot update itself yet,
  so it now shows when a newer version is available, with a Download button.
- **Keeping a compared answer keeps its model.** The conversation now carries
  on with the model you kept, instead of going back to the teammate's own
  model and starting a fresh session.
- **Comparing on a free model stays free.** A free teammate's comparison now
  starts on two free models, instead of adding a paid one nobody chose.
- **Ask on the side knows the turn still running.** A side question asked
  during a run now knows what that run was asked.
- The file preview's footnote no longer says "Locust does not open files"
  under the file it is showing.

## 0.514.0 - 2026-10-01

### Changed

- **Starting free is one press.** Home's "OpenCode works without one" now
  ends in **Use a free model**, which puts the chat box on OpenCode's free
  model. It shows only when the chat box is not already on OpenCode.
- **A new teammate's model comes first.** In the new-teammate form, and when
  editing one, the model sits right under the name, before how the teammate
  looks, its role and its connectors, so you see what it will run on before
  you press Create.
- **Connectors have readable names** in a teammate's settings ("Zoho
  Projects", not `plugin:small-business:zoho-projects`); the full name is
  still there on hover.
- **A review only reads.** "Ask … for a review" now runs that review in Ask,
  so the reviewer cannot change your files. It is for that review only: the
  reviewer keeps the mode you gave them.

### Fixed

- **A copy of Locust leaves the machine's agents alone.** A copy that is not
  the installed Locust no longer updates Codex CLI or Copilot CLI on its own,
  and its Settings says so instead of showing a switch that would.

## 0.513.0 - 2026-10-01

### Fixed

- **A long conversation can always move to another model.** Moving a
  conversation to another runtime, handing over a stopped run or resuming
  one carries the task with it, and a task too long for that was cut short,
  or refused with "too long to carry to another runtime". That is the
  conversation most likely to have hit a limit. Now a long task is written
  whole to a file in the project's `.locust` folder (which git ignores), and
  the next model is told to read it first, so nothing of it is lost.

## 0.512.0 - 2026-09-30

### Fixed

- **A reply that mentioned one of Locust's tags lost words.** A teammate
  that wrote a tag's name in passing, like `<locust-file>` in backticks, and
  then handed over a real file had everything between the two deleted from
  its message, and the file button never appeared. The message now keeps
  every word, and the real block is found.
- **A file handed over from outside the folder is said, not dropped.** A
  teammate pointing at a file outside the folder the conversation works in
  got no button and no word. The thread now names the path and says why it
  is not opened from there.
- **Putting files back is all of them or none.** Editing an earlier message
  with "Also put back" ticked could undo some files and leave others whose
  change was not recorded exactly, which can leave a project half changed,
  with its own tests failing. Now, if any file cannot go back exactly,
  nothing is put back and the thread says which file stopped it. When Locust
  knows this before you send, it does not offer to put files back at all.
- **"Show the edited version" is there straight away.** Going to the version
  before an edit, in the same session as the edit, showed the old branch
  without the way back until Locust was restarted.
- **An edit stops what the set-aside replies had asked for.** A message a
  teammate sent to another in the replies an edit set aside, still waiting
  to be read, could open that teammate's next run. It is no longer delivered.
- **A routine says which folder it runs in**, on its card and in its editor.

## 0.511.0 - 2026-09-30

### Fixed

- **When the model's servers give out, one press carries on.** A long run
  that ended with "Selected model is at capacity" (or the servers being
  overloaded or unavailable) left a red card with nothing to press, after
  an hour of work. The card now says it was the provider, not your account
  or anything you did, and offers **Continue**: it picks up the same
  conversation where it stopped, with everything it finished kept.

## 0.510.0 - 2026-09-30

### Removed

- **Finances, for now.** It read statements you downloaded and dropped in a
  folder, and a finances page is only worth having when it keeps itself up
  to date. That means linking bank accounts, which Locust will not do
  without doing it properly. The switch, the sidebar row and the page are
  gone. Nothing of yours was deleted: if you turned it on, the Finances
  teammate stays as an ordinary teammate, and your statements stay in their
  folder (`.locust/places/finances` in your home folder).

## 0.509.0 - 2026-09-30

### Fixed

- **A cloud task can always be found again.** A task sent to Codex Cloud
  makes no conversation, so once its panel was closed nothing in the window
  said it was there. The sidebar now has a Cloud tasks row, with how many
  this folder has, whenever it has any; it opens the panel.

## 0.508.0 - 2026-09-30

### Fixed

- **The release check tests the real update feed again.** Since 0.507 a copy
  of Locust that is not the installed one cannot update itself, and that
  included the copy the release check runs. It now asks the update feed
  without downloading or installing anything, so every release is still
  checked against what was actually published.

## 0.507.0 - 2026-09-30

### Fixed

- **A second copy of Locust no longer closes yours.** Every copy of Locust
  shares one update download, and a copy that was not the installed one (a
  tester's copy, or one run for testing) installed that update when it quit.
  The installer closes the Locust running from the installed folder, so your
  own Locust shut down mid-run with no error, and sometimes reopened on its
  own. Now only the installed Locust checks for, downloads and installs
  updates; other copies say they cannot update themselves.

## 0.506.0 - 2026-09-30
<!-- big -->

### New

- **A Finances dashboard.** Finances in the sidebar now opens a page, the
  way ChatGPT's own Finances does:
  - spend by category for any month, as a bar and a list, with the total;
  - money out and money in, compared with the months before;
  - subscriptions and bills, with roughly when each is next due;
  - fees and interest;
  - every transaction on its own tab.

  Ask a question at the top, or press a category, and the Finances teammate
  answers. It reads every CSV statement you put in the Finances folder,
  whatever shape your bank exports. Net worth, holdings and a credit score
  need linked accounts, which Locust does not have, so they are not shown.

### Fixed

- **Cloud mode no longer shows the "continues from checkpoint" hint**, which
  is about switching models and does not apply to a cloud task.

## 0.505.0 - 2026-09-30

### Improved

- **Cloud is one message's choice.** Picking Cloud used to stay picked, so
  the next ordinary message in another conversation went to Codex Cloud
  instead. The box now goes back to Direct when you switch conversation or
  teammate, and after a cloud task is sent.
- **A repository with no cloud environment is said before you send.**
  The Cloud tasks panel checks whether Codex Cloud has an environment for
  this folder's repository. If it does not, it says so, says how to make
  one, and lists your folders that have one, instead of refusing after you
  press send.

## 0.504.0 - 2026-09-30

### Improved

- **Cloud says where it can go.** Cloud could not be picked in a folder that
  is not on GitHub, and the menu said only that. Now Cloud can always be
  picked on a Codex model, and the Cloud tasks panel names the folder, says
  what the cloud needs, and lists your folders that are on GitHub, each with
  **Open**. **Choose a folder…** is there for one Locust has not opened yet.

## 0.503.0 - 2026-09-30
<!-- big -->

### New

- **Cloud tasks.** Pick **Cloud** in the chat-type menu (beside Direct,
  Compare and Blind), describe a task, and it runs in Codex Cloud on this
  folder's GitHub repository, using your cloud credits or plan. A Cloud
  tasks panel beside the conversation follows it until it is ready. **Show
  the change** draws its diff, and **Apply to this folder** brings it in,
  uncommitted, so you can look before you keep it. Nothing comes into your
  folder before you apply.
  Before a task starts, you are told what the cloud will not see:
  commits not yet on GitHub, and uncommitted changes. It needs a Codex
  model, and an environment for the repository made in the Codex app under
  Settings > Legacy Codex Cloud; if one is missing, Locust says so.

## 0.502.0 - 2026-09-30

### New

- **An edit can put the files back too.** When you edit an earlier message,
  the box offers "Also put back the N files the replies after it changed".
  It is off unless you tick it. Ticked, each of those files goes back to how
  it was before those replies, but only where Locust's record of the
  changes is complete and the file has not been changed since. Any other
  file is left as it is, and the conversation names it and says why.

## 0.501.0 - 2026-09-30
<!-- big -->

### New

- **Finances.** Switch it on in Settings › Connectors, and Finances appears
  in the sidebar. It is a folder of your own, outside every project, with a
  teammate on Codex who only reads. Drop your bank or card statements in
  (CSV or PDF exports) and ask where the money went, what you pay for every
  month, or what changed since last month. If you linked accounts in
  Codex's own Finances, the teammate can use those too. Locust never asks
  for bank details, links nothing itself, and cannot move money.

### Improved

- **A Codex release Locust cannot read is held back.** Locust updates Codex
  on its own, and each new Codex release is now first run through a real
  turn and read by Locust. A release that fails that check is not installed
  on its own; its row in Settings says so, and Update still installs it if
  you want it.

## 0.500.0 - 2026-09-30

### Fixed

- **An edited message is never queued.** If the teammate was busy in
  another conversation when you sent an edit, it was queued like a reply
  and would have gone out at the end of the conversation, not where you
  edited it. It now stays in the box and says the teammate is still
  working; send it once they finish.

## 0.499.0 - 2026-09-30

### Improved

- **The version before an edit is still there.** A message you edited says
  "Edited", with **Show the version before**, which opens the conversation
  as it was, replies and all. That version says "This message was edited
  later" and links back. Both stay under one conversation in the sidebar,
  and still say so after Locust restarts.

## 0.498.0 - 2026-09-30
<!-- big -->

### New

- **Edit an earlier message.** Point at any message you sent and press the
  pencil beside it. Its words go back in the box. Change them and send, and
  the conversation starts again from there, as Claude Code's rewind does.
  The teammate is given everything said before that message and nothing
  after it. The conversation stays one row in the sidebar; editing your
  first message starts a new one and keeps the old. Files the later replies
  changed are left as they are. Antigravity can only rewind from the first
  message, since it carries on only its own conversation.

## 0.497.0 - 2026-09-30

### Improved

- **Smaller again.** Locust carried a second copy of the libraries its
  window is already built from, and nothing ever read it. About 15 MB less on disk,
  and nothing you use changes.

## 0.496.0 - 2026-09-30

### Improved

- **Send again reads as one message.** Stop a reply before it does anything,
  press Send again, and the thread now shows your message once, answered,
  instead of twice with "A fresh session" between them. The teammate is no
  longer told about an "earlier turn" that was the same words with no reply.
- **A reply after an early Stop keeps the conversation.** When you stopped a
  reply before it got going, the next message started the teammate from
  scratch. It now picks up the same session, the way Claude Code does after
  an interrupt, and the stopped message says "Stopped before it replied"
  instead of sitting there as if still waiting.
- **A card for a stop before any tool ran is shorter.** It no longer says
  that undoing changes is up to you, since no change was made.
- **The Mac build is started on a real Mac before it is handed out.** Each
  Mac build now runs one reply on a free model and presses Stop, and checks
  that nothing it started is left running.

## 0.495.0 - 2026-09-30

### Improved

- **A reply that cannot pick up its session is told the conversation so
  far.** When the turn before stopped too early to be continued, or you
  changed the mode, the teammate used to start with no idea of what came
  before, and the thread said "Started without the earlier messages", which
  read as if your history was gone. The teammate is now given what was asked
  and answered, and the thread says so.
- **"Could not be reached" is told apart from "signed out".** When a
  runtime's own service does not answer, as Cursor's did for a while, Locust
  now says it could not be reached and to try again, instead of telling you
  to install or sign in.
- **Enter picks the one model left** when you search the model list down to
  a single row.
- **Model rows read cleanly**: a free OpenCode model says "Free · no sign-in"
  and others "Through OpenCode", and a screen reader hears the model's name
  and state once, not every word on the row.
- **About 48 MB less on disk.** Locust no longer installs Chromium's
  interface text in 53 languages it does not use.

### Fixed

- **An example file is no longer drawn as a file handed over.** A model that
  copied the example from its instructions left a `path/relative/to/the/folder.md`
  button under its reply, pointing at nothing.

## 0.494.0 - 2026-09-30

### Fixed

- **A step keeps the change it made.** After a turn, Locust looks at the
  folder and records each file's whole change over the run. That record
  replaced the change of the step that last edited the file, so a small
  two-line fix read as the whole file once the turn ended. Each step now
  keeps its own change.
- **A turn's files card lists each file once.** A file edited twice, or
  edited as part of a change to several files at once, could appear twice,
  with the total counting it twice ("Edited 10 files" over twelve rows). The
  card now has one row per file, showing its whole change over the turn.
- **Step lines count the files a command read**, across every part of a
  command that reads several, and a file listing no longer reads as a search
  for `**`.

## 0.493.0 - 2026-09-30

### Fixed

- **A comparison that changes files can no longer change your folder before
  you keep anything.** In a git project each model worked in a copy kept
  inside your folder, so the copy's own path led back to your project, and a
  model working in Auto edited your real file instead of its copy. Keep
  then refused. The copies now live outside your folder entirely.
- **A routine that is running keeps the steps it started with.** Editing a
  routine while it ran used to change steps that had not started yet, a
  checker included. Your changes now apply from the next run, and the edit
  window says so.
- **A routine runs on its teammate's model as it is now.** A routine kept
  the model its teammate had when the routine was made, so a teammate moved
  to Grok 4.7 still ran its routine on 4.6. It now uses the teammate's
  current model, in the mode the routine was saved with.
- **In Compare, the model chips no longer lie over the other controls** at
  some window sizes, where pressing Ask opened the model picker instead.
- **Every Compare column is named with its effort level**, the default one
  included. A level you left alone used to go unnamed.

### Improved

- **The routine list names the model each teammate's steps run on**, and
  the edit window says the model and the mode it runs in, instead of the
  same line about read-only on every routine.
- **While a teammate works through a plan, the live line says which step
  it is on** ("step 2 of 3"), so a long run's plan is not only at the top of
  the turn, scrolled out of view.
- **A thought on its own line opens straight onto its words**, without a
  second row repeating its headline.
- **Clearer step lines**: listing files reads "Listed every file" instead
  of "Searched for **/*", and Codex running JavaScript reads "Ran
  JavaScript".

## 0.492.0 - 2026-09-30

### Improved

- **Long stretches of work read as the moves they were.** When a teammate
  works a long while without saying anything, its steps no longer pile into
  one line. A new line starts each time the model stops to plan its next
  move, the way Codex's and Antigravity's own apps show it.
- **Codex's thinking is shown under its own headline.** Each time Codex
  thinks, its line leads with what it was thinking about ("Planning the
  fix: read app.ts, ran npm test"), and opening the thought shows Codex's
  summary. Until now a Codex thought showed only how long it took.
- **Cursor's thinking says what it was about** when a thought is the whole
  line, instead of the single word "Thought".
- **Clearer step lines.** A script is "Ran a script" or "Ran a node
  script" rather than its first cryptic line, one command reading two files
  reads "Read 2 files", a search names what it looked for or the folder it
  searched, and "searches" is spelled right.

### Fixed

- **A helper sent to the background no longer says it reported back the
  moment it starts.** Claude Code's background helpers now read "working in
  the background", with what they are doing, until they come back, and then
  show what they came back with.
- **A step whose result was too large no longer says it did not report.**
  When Claude Code returned something over 256 KB, such as an image or a
  whole file, Locust could not keep it, and the step read "did not report" as
  if Claude Code had gone quiet. It now says the result was too large to keep,
  and how large.

## 0.491.0 - 2026-09-30
<!-- big -->

### Improved

- **A teammate's turn reads the way Claude Code's does.** What the teammate
  says and the steps it takes now appear in the order they happened. After
  each thing it says, the steps it took before saying the next thing sit on
  one quiet line, such as "Read 2 files, ran npm test" or "Created
  report.md". Press the line to see each step, with its command and output
  or its change. Until now, every step of a turn went into one bar of counts
  above everything it said. The bar stayed closed while the teammate worked,
  and its words piled up underneath, cut off from the steps they described.
  Nothing moves when a turn ends: it reads the same while it runs and after.
  A step that failed says so in amber on its line, and a single described
  command that failed reads "Failed to ..." as it does in Claude Code.
- **A finished turn ends with what it changed.** The files it edited are
  listed in one "Edited N files" card, each closed until you open it, with
  the turn's totals (how long, what ran, what went wrong) on a quiet line
  under it.
- **Files that only changed around a teammate are no longer counted as its
  edits.** In a folder another program writes to, such as your `.claude`
  folder, where Claude Code keeps its own backups, the card now reads
  "Edited 1 file · 12 more changed in the folder" instead of "Edited 13
  files".

## 0.490.0 - 2026-09-30

### New

- **Compare has an effort for each model.** Setting up a comparison, each
  model's chip now carries its own effort beside the name, as one chip in
  two parts: press the level to open the same slider the single effort chip
  has. Two or three models are compared at the levels you choose rather than
  each at its default, and a column is named with its level when you chose
  one, so Opus at High and GPT at Low read apart. A model that lists no
  levels shows none, and a Cursor model runs the id its level names.

## 0.489.0 - 2026-09-30

### Security

- **Locust no longer shows or keeps files that hold credentials.** After
  each run, Locust shows the files that changed in the folder, with what
  changed in them. When a teammate worked in a folder that also holds login
  files, such as your `.claude` folder, a login file refreshed during the
  run was shown with its tokens and kept in Locust's history. Files that
  look like credentials (login and key files, anything named for auth,
  tokens, secrets or sessions, and dotfiles) are now listed by name only and
  never opened. Anything else shown is scrubbed of keys and tokens first. If
  you ran a teammate in a folder like that, sign out of the tool whose login
  was shown and sign back in.

### Fixed

- **A command's output shows under it, for Claude and Codex.** As in their
  own apps, opening a command's row now shows what it printed. It showed
  only for Antigravity before.
- **"Thought for 12s" shows even when a runtime keeps its thinking
  private.** Claude and Codex now leave a thinking line, as Claude Code
  does. The thinking itself is never kept.
- **Codex's live line no longer says "Thinking" through the whole answer**,
  or "writing" when it is your own message arriving.

## 0.488.0 - 2026-09-30

### Changed

- **Missions, Rooms and Routines sit in one row** at the top of the sidebar,
  with a thin line between them, instead of three lines with empty space
  beside each.

## 0.487.0 - 2026-09-30

### Fixed

- **Antigravity turns no longer end while the agent is still working.** When
  Antigravity's agent put a command or a timer in the background and said
  "waiting for it", Locust called the turn finished. The agent then carried
  on in Antigravity's window, and none of that work showed in Locust. A turn
  now stays open until its background work has ended. On your own Boss
  conversation, this took the turns wrongly marked finished from 15 to none.
- **Antigravity's background commands and timers say how they ended.** They
  used to read "did not report". Now each shows as running in the
  background, then done, failed (with its exit code), stopped, or, for a
  timer, gone off. On the same conversation, tool calls left open went from
  55 to 7.
- **Antigravity command rows show the command.** They showed the agent's
  summary of it, such as "Checking commit 8b104218". The summary is now the
  row's title, and the command itself is shown.

## 0.486.0 - 2026-09-30

### Changed

- **The sidebar is set out like Claude's.** Missions, Rooms and Routines are
  now a list at the top, one to a line. Project names are plain headings,
  with no folder icon and no count. Conversations outside a project sit
  under "Ungrouped", and rows no longer show how long ago each one moved.
  Settings stays at the bottom.

### Fixed

- **A brief from one teammate to another arrives whole.** Messages between
  teammates were cut at 1,200 characters, about two paragraphs, so a real
  brief arrived ending mid-word, and the teammate who got it said so. They
  now carry up to 6,000 characters, about 1,000 words, and teammates are
  told that limit.

## 0.485.0 - 2026-09-30

### New

- **Stop and send now.** A message you type while a teammate is working
  waits until they finish. It now has a Stop and send now button, which stops
  the work in progress and sends your message at once in the same
  conversation. Ctrl+Enter does the same as you type, like Claude Code's.

### Fixed

- **Grok and other Cursor models work in Compare on Windows.** An
  answers-only comparison made Cursor work in a copy of the folder, so in
  any big folder its column said "could not start". Cursor now runs in its
  own read-only ask mode, in the folder itself. When pushed to write, it
  refused every time we tried. The same makes Ask and Plan available for
  Cursor on Windows.
- **Replies are set in Anthropic Sans**, the face of the Claude app, instead
  of the serif.
- **A long tool name no longer covers "Using a tool".** In a narrow column
  the name of what was running, such as a whole PowerShell path, was drawn
  over the words beside it. Now the name shortens instead.
- **A comparison with one column still answering says "Ask a follow-up",**
  not "Ask all 1".
- **A stuck Cursor check no longer lingers after Locust closes.** When Locust
  asks Cursor which connectors it has and Cursor hangs, Locust gave up
  waiting but left Cursor's process running, sometimes long after the app
  closed. Now it ends the whole process.

## 0.484.0 - 2026-09-30

### New

- **Point at part of a page and ask about it.** When a teammate makes a web
  page, open it and press the target button above it, then click any part of
  the page. The chat box gets a quote naming the file, where that part is and
  its code, with a picture of it attached. Nothing is sent until you send it.
  The click never reaches the page itself, and Esc or the button again stops
  pointing.

### Fixed

- **The chat box grows with what you write.** It was always one line tall,
  so a pasted paragraph or a quoted part of a reply sat behind a small
  scroll arrow. Now it grows up to a few lines, then scrolls.

## 0.483.0 - 2026-09-29

### Fixed

- **The model picker names OpenCode's models.** Its rows showed ids like
  `ling-3.0-flash-fin-free`, while the chip beside it said "Ling 3.0 Flash
  Fin". Now the picker uses the names too.
- **The folder chip keeps its icon.** When the chat box row was tight, the
  folder icon shrank to a dot. Now the folder name shortens instead.
- **The narrow sidebar's buttons stack.** In a narrow window, Missions, Rooms
  and Routines were three small icons squeezed into one row, and Settings sat
  alone at the edge. Now each has a row of its own, centred like the
  teammates above it, and is easier to click.

## 0.482.0 - 2026-09-29

### Changed

- **A web page a teammate made can no longer send your files anywhere.** A
  page opened in Locust can read its folder, which is what lets a page chart
  your `data.csv`. It could also send what it read to any site. Now it can
  only load libraries, stylesheets and fonts from the common public hosts
  (cdnjs, jsDelivr, unpkg, Google Fonts and a few others), the way Claude's
  own artifacts work, and can reach no other site. When a page opens a
  website in your browser, Locust shows you the address and asks first. A
  page that calls a live web service, or embeds a video, will no longer
  load that part.
- **Feedback says it is public.** A report opens as a public issue on
  GitHub, and the form now says so. Your conversation goes with it only if
  you tick "Include this conversation", which starts unticked.
- **The message box shows focus a little more clearly.** Its edge is still
  a single thin line with no colour, now bright enough for the
  accessibility minimum.

## 0.481.0 - 2026-09-29

### Fixed

- **A run that fails because you are signed out offers Sign in.** When
  Codex's saved sign-in expired, the card said to log out and sign in again
  but had nothing to press. Settings still said ready, because Codex's own
  check only looks for a saved sign-in. Now the card has the same Sign in
  button as Settings, for Codex, Copilot and any runtime that reports it.

## 0.480.0 - 2026-09-29

### Fixed

- **A folder is one folder however you reach it.** Opening a project as
  `c:\work` after `C:\work`, or through a link or junction to it, made it a
  second folder with none of its conversations, so your history looked gone.
  Now Locust knows it is the same folder and keeps it as it was first known.
- **Question cards grow with the reply text size.** At Large and Largest the
  reply grew, but a teammate's question card and your own messages stayed
  small. Now they grow with it. The rest of the app stays as it is.

## 0.479.0 - 2026-09-29

### Fixed

- **A run you stopped before it did anything offers Send again.** The card
  said nothing was half-done and left the chat box empty, so the message had
  to be typed again. Now one press sends it as it was. It is offered only when
  no tool had run, so it cannot repeat anything.
- **A free model that gives up on its limit offers the next one.** The note
  and its button left when the run ended, and the red card said to pick
  another model with nothing to press. Now the card has a Switch to <model>
  button, which puts your message back in the chat box to send there.
  Claude's limit and the other paid runtimes still leave the choice to you.
- **"ledger verified" now says "ledger readable".** The check is that every
  file reads and none has a gap in it. An edit made while the app was closed,
  or a whole file deleted, passes that check, so "verified" promised more than
  it checks.

## 0.478.0 - 2026-09-29

### Fixed

- **Signing in works on a Mac.** Sign in, in Settings, opens Terminal with
  the runtime's own sign-in, as it opens a window on Windows.
- **A Mac says it is a Mac.** Feedback from a Mac named it as Windows, and a
  file's Reveal said it would hand the file to Windows. Both now say macOS
  and the Finder there.

## 0.477.0 - 2026-09-29

### New

- **Open in terminal works on a Mac.** The `</>` button in a conversation's
  header opens Terminal in the conversation's folder, carrying on the same
  session, as it opens Windows Terminal on Windows.

### Fixed

- **A damaged record says so in the conversation.** When part of a turn's
  record could not be read, the reply stopped mid-sentence with nothing to
  say why. The conversation now says what is missing and that the file is
  kept as it is.

## 0.476.0 - 2026-09-29

### Fixed

- **Stop ends everything on a Mac.** On macOS, Stop ended the coding tool
  but could leave what it had started running, a dev server or a long
  command. Each run now stops together with everything it started, as it
  does on Windows.
- **A switch of runtime is described as it happened.** Replying on another
  runtime after a finished turn told the new one that the last agent
  "stopped partway through". It now says it is taking over the conversation.

## 0.475.0 - 2026-09-29

### New

- **Import a conversation from Claude Code or Codex.** From the + menu,
  Import a conversation lists the sessions you had in Claude Code (terminal
  or its app) and Codex over the last 30 days, with their folders. Choose
  one and its exchanges come in as a Locust conversation; your next message
  continues that same session, in its own folder. A session still open in a
  terminal says so. Only the words come in; the steps it took stay in the
  session.
- **Ask about a part of a reply.** Select words in a teammate's reply and
  choose Ask about this: they are quoted into the message box, so your next
  message is about that part.

## 0.474.0 - 2026-09-29
<!-- big -->

### New

- **Projects, like Claude's.** The groups you filed conversations under are
  back, as projects, drawn with the folder look: a folder icon, the project's
  name, and its count. A project can hold conversations from any folder.
  Make one from the + menu (New project), or file a conversation from its
  menu (Move to project). The folder a conversation works in is still the
  one in the chat box, where you switch it.
- **Math is drawn.** Equations in a reply, written `$...$` inside a sentence
  or `$$...$$` on their own line, are drawn as math instead of showing the
  raw LaTeX. Prices like "$5 and $10" stay as they are.

### Fixed

- **A one-letter emphasis is italic.** "Column *j*" showed its asterisks.

## 0.473.0 - 2026-09-29

### New

- **A first build for Mac, for testing.** Locust now builds for macOS, on
  Apple silicon and Intel. It is not signed by Apple yet, so the first time it
  opens macOS asks: choose Open Anyway in System Settings, Privacy &
  Security. A Mac copy does not update itself yet; each new build is
  downloaded and replaces the old one. On a Mac, Locust finds the coding
  tools your Terminal finds, and clicking it in the Dock brings back a
  window that was closed to keep working.

## 0.472.0 - 2026-09-29

### Improved

- **The sidebar is by folder again, in the folder look.** Each folder heading
  has its folder icon and its name as the folder spells it, as before 0.465,
  and a single folder is headed too. The old groups, retired in 0.460, are no
  longer drawn; your groups file is left as it is.

### Fixed

- **A runtime that crashes says why.** When Codex's or OpenCode's background
  server died mid-answer, the card quoted Locust's own sentence back as "the
  runtime's last word". It now shows the last thing the runtime itself said.

## 0.471.0 - 2026-09-29

### Fixed

- **A double click approves one card, not two.** When a teammate asked again
  right after an approval, the second click of a double click approved the
  new card unread. A card now takes no approval in its first half second.
- **`@` finds a file whatever its accents.** `@resume` now finds `résumé.md`.
- **A memory says how long it can be.** The memory box stopped taking
  keys at 300 characters with nothing saying why; it shows a count as it
  nears the limit.
- **The side panel says what a side question can do.** It said nothing
  there changes a file; it now also says your connectors still work in it.

## 0.470.0 - 2026-09-29

### Fixed

- **"Trying again" leaves when it stops being true.** The amber note that
  OpenCode was retrying a busy model stayed above the error after the run
  ended, and above the answer when the model came back. It now goes once the
  run ends or the model answers.
- **A failure card ends in words, not a log line.** When OpenCode stopped on a
  busy model, the card ended in its raw log line. It now says the error itself.
- **The model picker no longer points to a fallback setting that does not
  exist.** Its footer said "Fallback chain, privacy and permissions live in
  Settings"; Settings has no fallback chain yet.

## 0.469.0 - 2026-09-29

### Fixed

- **A teammates file that will not read is said, not hidden.** Home showed the
  first-launch "Start with a team", so the team looked gone. It now says the
  file could not be read and that nothing was changed, and adding a teammate
  names that as the reason rather than "check the name, hue and role".
- **A routine whose checker was removed is refused before it spends.** It
  used to run its first steps and then stop, naming nobody. It now refuses
  at the start and says which step lost its teammate.
- **A long task can follow a teammate to another runtime.** A task and a
  reply that together passed about 7,700 characters were refused. The start
  of the original task is kept, the briefing says how much of it was left
  out, and your new message is carried whole.
- **No "Try again" for a folder too big to copy.** A comparison in a folder
  too big to copy offered Try again, which could only give the same answer.

## 0.468.0 - 2026-09-29

### Fixed

- **Codex thinking reads "Thinking".** A Codex run that was only thinking said
  "Using a tool…" the whole time.
- **Tool names keep their underscores.** `mcp__github__create_issue` in a
  reply was drawn as mcp**github**create_issue.
- **Removing a working teammate stops their run.** Their run used to carry on
  with no owner. The menu now says "Remove, and stop what they are running?"
  and stops it.
- **A teammate's card closes when you change screens.** A card pinned from the
  narrow sidebar stayed open over Missions, Settings and the rest, and could
  catch a click meant for the screen.
- **Files past the limit are named.** A message carries 8 files; choosing 10
  kept 8 and said nothing. It now says which were not attached.
- **A file name cannot add a line to the brief.** A name with a line break in
  it (possible on macOS and Linux) is quoted in the list of attached files.
- **The spellchecker downloads nothing.** Where the window spellchecks with a
  downloaded dictionary, it no longer fetches one from the internet.

## 0.467.0 - 2026-09-29

### Fixed

- **"Always allow" says what it allows, and allows less.** On Claude, Always
  on one connector tool used to let every tool of that connector through for
  the rest of the reply, deleting ones included. It now covers the one tool,
  and every card says what Always would let through: the tool, every command,
  or OpenCode's own pattern such as "echo *".
- **A question on the side can be answered and stopped from its panel.** An
  approval its answer waited on showed only in the title bar; it is in the
  panel now, with a Stop while it answers. Closing the panel stops it, and a
  side question no longer appears in Missions as a conversation of its own.
- **A subagent's request is asked.** When an OpenCode teammate in Approve
  each handed work to a subagent, the subagent's request was never shown and
  the run waited on it forever. It is asked like the teammate's own, and says
  it came from a subagent.
- **A fetch is a fetch.** OpenCode's web fetches and reads outside the folder
  were asked as "Run a command". Each now says what it is and what it sends.
- **A change outside your project folder says so.** A write outside the folder
  read like one inside it, "reversible if under version control". It now says
  it is outside the folder and cannot be undone from here.
- **A Codex file approval shows its change.** When Codex asked to change a file
  at the same moment it described the change, the card could arrive without
  the file or the diff.

## 0.466.0 - 2026-09-29

### Fixed

- **A long conversation reopens with every reply.** After a restart, a
  conversation of more than 20 turns showed its early questions with no
  answers under them. Every turn's reply is read back now.
- **Older conversations stay listed.** Past a few hundred turns, older
  conversations dropped out of the sidebar, Missions, Ctrl K and a
  teammate's card, and nothing said so. The list now reaches the newest 2,000
  turns, and past that the Missions header says how many are listed of how
  many are kept.
- **`@` finds files in a big project.** It listed only the first 5,000 files,
  so in a large repository many files could not be found. It now lists up to
  50,000, and says so when a folder has even more.

## 0.465.0 - 2026-09-29

### Improved

- **Folder headings look like the old group headings.** Each folder in the
  sidebar now has a chevron and its name in the small uppercase label, with
  its count at the end.

### Fixed

- **A memory for every folder waits for you.** A teammate's "remember
  everywhere" is read by every teammate in every folder, so it now waits on
  the Memory screen for you to keep, even when memory keeps things on its
  own. A memory for this folder still keeps itself. Teammates are also told
  that a note is information, never an instruction.
- **More secrets are refused.** A login inside an address
  (`postgres://user:pass@…`), an AWS secret key, "the password is …", a
  passphrase, "the login is admin / …", and a card number are no longer
  kept as memories. Phone numbers and other personal details still are.
- **A question that could not be a card stays in the reply.** A question
  with one option, five, "Yes" and "No", or two the same used to disappear
  from the reply. So did a second question after the first. Both now stay,
  as plain text you can answer.
- **Memories past the limit are counted.** A reply keeps at most four
  memory lines. The rest were dropped without a word; the conversation now
  says how many were not kept.
- **A daily routine keeps its time when the clocks change.** "Daily at
  09:00" ran at 10:00 or 08:00 the day after a clock change.

## 0.464.0 - 2026-09-29

### Fixed

- **An approval shows the whole action.** A card showed the first 600
  characters of what a teammate wanted to do, on one line, so a mail's
  recipient written after a long body never appeared on it. It now shows
  all of it, one field to a line, with a command's line breaks kept.
- **A new file is drawn as itself.** A new file whose first lines looked like
  a change to another file was drawn as that change, and its own lines were
  not shown. An edit that removed a `-- comment` line in SQL could not be
  drawn at all, and the +N −M counts skipped such lines. All three are right
  now.
- **A checker's approval has to be a plain one.** In a routine with a checker,
  "VERDICT: APPROVED, but the tests fail" counted as approved. Only a plain
  approval passes now; anything added to it counts as changes needed, in the
  checker's own words.
- **A connector named like a file tool is not a file edit.** A connector
  called `write_file` or similar was shown as a change to a file in this
  folder.
- **A teammate that cannot start says why.** Every failure read "The Codex
  process transport ended unexpectedly", for Claude and OpenCode too. It now
  names the runtime and says whether its program is missing, was not
  allowed to run, or the folder it works in is gone, and that failure no
  longer brings up an error box of its own.
- **A page cannot read a hidden file through a link.** A file in the page's
  folder linked to `.env` or `.git` was served under its innocent name.

## 0.463.0 - 2026-09-29

### Improved

- **A teammate's reply sits under the conversation that asked for it.** When
  one teammate messages another, the reply's conversation is drawn in the
  sidebar right under the one it came from, instead of as a stranger beside
  it.

### Fixed

- **"Open Wren's conversation" works after a restart.** A reopened
  conversation lost its link back to the teammate's conversation that sent
  it; the link now survives.

## 0.462.0 - 2026-09-29
<!-- big -->

### New

- **Ask on the side.** From a conversation's ... menu, ask about it while
  it works -- why it chose something, what it has done so far. The answer
  comes from a copy of the conversation on the same model, read-only, in a
  panel beside it; the conversation keeps working and never sees the
  question. Follow-up questions in the panel remember the ones before.
  Works with Claude Code, Codex and OpenCode.

## 0.461.0 - 2026-09-29

### Improved

- **Build and compare, drawn properly.** Each starter on Home now shows
  what it makes -- a browser window, a chart, a gamepad -- on its own tile,
  and the three are more fun to watch two models attempt: a coffee shop's
  landing page, a sales dashboard, and a neon Snake game. The row now says
  what it does: "Try two models on" a landing page, a dashboard or a game.

## 0.460.0 - 2026-09-29

### Improved

- **Compare picks its models like Arena.** Switching the message box to
  Compare starts on two models already chosen, each in its own dropdown:
  press one to change that model alone, or + to add a third. It was one
  "A vs B" button opening a checklist.
- **Build and compare starts in one press.** The three starters on Home are
  cards now, each with a line about what it makes. Pressing one puts its
  words in the box and two models beside them; Send starts both.
- **Folders are how conversations are sorted.** Since folders appear in
  the sidebar by project, groups did the same job twice, so new groups are
  no longer made. The groups you have stay, inside their folder, and Move to
  group still takes a conversation out of one.

## 0.459.0 - 2026-09-29

### Improved

- **A finished command says how long it took.** A command a teammate ran
  now shows its time at the end of its row -- "4s", "1m 35s" -- so a slow
  test run or build stands out. Commands under a second, ones sent to the
  background, and ones that never ran say nothing.

## 0.458.0 - 2026-09-29
<!-- big -->

### New

- **Folders work like Claude Code.** Switching folders no longer restarts
  Locust: nothing closes, and runs keep going where they are.
- **Every folder's conversations in the sidebar, by project.** Each
  conversation is listed under the folder it belongs to, the one you're in
  first. With a single folder the sidebar looks as it always did. Your
  groups stay inside their folder.
- **A conversation stays in its folder.** Opening one from another folder
  takes you there, and a reply runs in that conversation's folder, whichever
  folder you were in before.
- **The folder button is a menu.** It lists the folders you've worked in, plus
  "Choose a folder...". Picking one starts a new conversation there.

## 0.457.0 - 2026-09-29

### Fixed

- **Compare says up front when a folder is too big to copy.** In a folder
  that is not a git project and holds more than 5,000 files or 250 MB,
  choosing Auto in Compare started both models and both came back "could
  not start". Now Auto is greyed out in the Compare menu with the reason,
  and the comparison answers instead.
- **An option you cannot choose looks it.** In the permission and Compare
  menus, an option that is not available here now shows dimmed, with its
  reason underneath, instead of looking like any other.

## 0.456.0 - 2026-09-29

### Fixed

- **A very long Claude answer ends properly.** An answer over about 250 KB
  kept the run going after it had finished, until you pressed Stop. It ends
  by itself now.
- **Double-clicking Start no longer stops the run.** Start and Stop are the
  same button, so the second click of a double-click stopped the run
  before anything happened. A Stop pressed in the first moment after
  Start is now ignored; press it again to stop.
- **Compare in Auto says what a copy does not do.** Each model still works
  in its own copy and only the one you keep comes in, but in Auto a model
  can still change files outside its copy, and the menu now says so.

## 0.455.0 - 2026-09-29

### Fixed

- **A page a teammate makes can no longer read your secrets.** A web page
  runs by itself in Locust, and it could read any file in your project --
  a `.env` with keys in it, say -- and send it anywhere. Now a page is
  served only its own folder, and never a hidden file (`.env`, `.git`) or
  anything shaped like a key, even there. Its styles, scripts, images and
  data still load as before. A page that reaches up a folder
  (`../css/site.css`) loses that file in the preview; in a browser it
  still has it.
- **Long links wrap.** A long URL, hash or path in a reply ran off the
  right edge and made the conversation scroll sideways. It wraps now.

## 0.454.0 - 2026-09-28

### Improved

- **Memory finds notes by meaning.** When a teammate starts, the notes it
  is handed are the ones closest in meaning to what you asked, not just the
  ones that share a word with it: "how do I publish the website?" now finds
  "the site deploys from the gh-pages branch". Checked against 136 notes
  picked by hand for real questions, the brief now carries 84 of them where
  it carried 70. It runs on your computer with a small model that ships
  with Locust (about 20 MB more to download), and nothing is sent anywhere.
  If it cannot answer quickly, the brief is chosen the old way.

## 0.453.0 - 2026-09-28

### Fixed

- **One count per column.** In a comparison, a column's work summary no
  longer shows a running tally of every edit along the way beside the
  footer's count of what actually changed. The footer is what Keep would
  bring into your folder.

## 0.452.0 - 2026-09-28

### Fixed

- **Pages built in a comparison show for every model.** When Claude Code
  or Codex built the page, its column said "That page is not there" even
  though the page was there. Each column now finds its page in its own
  copy.

## 0.451.0 - 2026-09-28

### New

- **Direct, Compare or Blind, in the message box.** A chip beside the
  permission mode picks how you ask: Direct (one model), Compare (two or
  three of your choice, side by side) or Blind (the same, names hidden until
  you keep one). It replaces the switch inside the model picker.
- **Comparisons that change files run in Auto.** Each model works in its
  own copy and runs what it needs, tests included, without stopping to ask.
  Only the one you keep comes into your folder. Choosing Auto turns it on,
  as it does in the permission menu.

### Fixed

- **A comparison column waiting on you shows its approval card.** Before,
  the card only appeared under "need you" in the title bar, and the column
  just said it was using a tool.
- Files a tool makes while a model works, like Python's cache or
  `node_modules`, are no longer counted as its changes or brought into your
  folder when you keep it.

## 0.450.0 - 2026-09-28

### Improved

- **Built pages line up side by side.** When models build a web page in a
  comparison, each column now shows its page first, running, at the top.
  The pages sit level with each other however many steps each model took to
  get there, and the plan and commands follow underneath.

## 0.449.0 - 2026-09-28

### New

- **Compare blind.** Tick "Hide the names" when picking models to compare.
  The columns read Model A and Model B in a random order, with no runtime,
  logo or cost to give them away, until you keep one. Then the names show.
- **Your own record, while you pick.** On Compare, each model you have
  compared before shows how often you kept it, "kept 2 of 3", in place of its
  description. It counts only comparisons you decided, and it stays on this
  computer.

### Fixed

- The picker no longer says models answer without changing files when the
  comparison is set to Edit.

## 0.448.0 - 2026-09-28
<!-- big -->

### New

- **Build and compare, from Home.** Under your AI agents: a landing page, a
  dashboard or a small game. Pick one, tick two or three models, and each
  builds it in its own copy of your folder. You watch every version running
  side by side, then keep the one you like. Only that one comes into your
  folder.
- **Comparing work no longer needs a git project.** In any folder, set a
  comparison to Edit and each model works in a plain copy. Keep writes back
  only the files that model changed, and changes nothing if you have edited
  one of them yourself in the meantime.

## 0.447.0 - 2026-09-28

### Fixed

- **A conversation's header names the model the way the model chip does.**
  A Sonnet conversation's header said just "Sonnet" while the chip below it
  said "Sonnet 5.5"; both now say Sonnet 5.5.

## 0.446.0 - 2026-09-28

### New

- **Sonnet 5.5 in the picker.** Claude Code already runs Sonnet 5.5 when
  you pick Sonnet; the picker now says so, and Sonnet 5 moves to the older
  versions.
- **Pages built in a comparison run side by side.** Ask two models to make
  a web page in an Edit comparison, and each column shows its page working,
  not as code. The page you keep still runs in the conversation afterwards,
  from your folder.

### Fixed

- A long new web page opens running, like a short one, instead of folded
  away as a large file.
- A page a model named by its place in your folder, rather than by its full
  path, now opens; it used to say the page was not there.

## 0.445.0 - 2026-09-28
<!-- big -->

### New

- **Compare the work, not just the answers.** In a git project, set the
  mode to Edit while picking models to compare, and each model makes its
  change in its own copy of your project, side by side. Your folder is not
  touched while they work. Each column says how much it changed. Keep this
  one brings that change into your folder, uncommitted, the same as a
  teammate editing your folder, and the other copies are removed.
- If you have changed the same file yourself in the meantime, Keep says so
  and changes nothing, so your work is never overwritten.

### Fixed

- **A comparison's copies no longer pile up.** Emptying the trash now also
  removes the copies a deleted comparison was still holding.

## 0.444.0 - 2026-09-28

### New

- **Every column in a comparison has its own buttons.** Copy puts that
  model's answer on your clipboard. The arrows give one answer the whole
  width, with the others kept to a narrow strip at the side that still names
  them; press the strip to switch, or the arrows again to see every answer
  side by side.
- **Try again.** A column whose answer failed or was cut short, because a
  provider was down or Locust closed mid-answer, offers Try again where Keep
  would be. It asks that one model the same question again, and its new
  answer takes the old one's place. The other columns are left alone.

## 0.443.0 - 2026-09-28

### Improved

- **Grok and Gemini can join a comparison.** They reach Windows through
  Cursor, which cannot be held read-only here, so the picker used to refuse
  them. Now such a column answers in a copy of your folder and says so in
  its heading; nothing in your folder changes, and the copy is removed when
  you keep one.
- **A comparison you can read at a glance.** Each column is ruled off from
  the next, from its heading down to its Keep button, and the question sits
  centred over all of them instead of over the last column.

### Fixed

- **Claude's refusal notes say what happened.** A read Claude Code refused
  was one reaching outside your folder, and the note now says that; it used
  to say "this route allows edits" even in Ask.

## 0.442.0 - 2026-09-28

### Improved

- **Compare without a teammate.** A comparison no longer needs a teammate:
  from Home, **Compare models** beside the AI agents opens the model picker
  already switched to Compare, so the first thing a new person tries can be
  two models answering the same question side by side. In a conversation
  with nobody, the picker's Compare switch works the same way.

## 0.441.0 - 2026-09-28
<!-- big -->

### New

- **Compare models side by side.** In a teammate's conversation, open the
  model picker, switch it to **Compare** and tick two or three models: Fable
  and GPT-6 Astra, say, or two free ones. Your ask goes to each, and the
  answers come back in columns side by side, each saying how long it took
  and what it cost. A follow-up goes to every column. **Keep this one** stops
  the others and carries the conversation on with the model you kept; the
  comparison stays one click away.
- **Nothing changes while you compare.** Every column answers in Ask, so a
  comparison works in any folder and no two models can get in each other's
  way. A model that cannot be held read-only on this computer says so in the
  picker instead of joining.

## 0.440.0 - 2026-09-28
<!-- big -->

### New

- **Land a teammate's work on your branch.** Give a teammate its own branch
  and every turn it finishes is saved there; Review changes now has **Land
  on main** (or whichever branch you are on). It lands the whole branch as
  one commit of yours, under a message you can edit, and your own commit
  hooks run. Afterwards the teammate starts from what landed.
- **Nothing half-done.** Before the button is offered, Locust checks what
  would stop it and says so in its place: the teammate still working, your
  own unsaved changes in a file it would write, or a conflict with your
  branch. If one of your hooks refuses the commit, your checkout is put back
  exactly as it was.
- **Conflicts go to the teammate.** When your branch changed the same lines,
  **Ask Wren to resolve** starts the merge on the teammate's branch only
  (yours is not touched) and asks in its conversation. When that turn ends
  it is saved, and it can land.

## 0.439.0 - 2026-09-28

### New

- **Each turn on Own branch is saved as a commit.** When a turn ends for a
  teammate working on its own branch, whatever it changed is committed to
  that branch (`locust/<name>`), with your ask as the message and the
  teammate, route and outcome as trailers. The thread says what was saved.
  Your own branch is never touched, your pre-commit hooks are not run for
  these, nothing is pushed, and a new file over 50 MB is left out and named.
- **Review changes.** A teammate on its own branch has a Review changes
  button beside Activity: the branch against where it left yours, as one
  change or turn by turn. It only reads; merging is still yours to do.

## 0.438.0 - 2026-09-28

### New

- **Tag teammates from any conversation.** Type @ in the message box and
  your teammates are listed above your files. Tagging one loops them in:
  the teammate you're talking to still gets the message, and each one you
  tagged is sent it too, in a conversation of their own, with the latest
  answer from where you tagged them. On Home, with nobody picked, the
  message goes to whoever you tag.

### Fixed

- **A hand-off chain's checker reads the files itself.** A checker could
  refuse a right answer because the teammate before it couldn't read files
  in its mode. It's now told it can read the folder, and to treat the
  answer it's given as a claim to check.

## 0.437.0 - 2026-09-28

### Fixed

- **A routine's updates aren't drawn as warnings any more.** "Step 2 of 3"
  and "finished, approved by Sable" read as ordinary notes; only a routine
  that stopped or is waiting for you is in amber.
- **Opening a conversation from this session shows its teammate's mode.**
  A conversation a routine had just started could open showing Edit when
  its teammate works in Ask.

## 0.436.0 - 2026-09-28

### New

- **Type @ to attach a file from your project.** In the message box, @
  lists the folder's files, narrowing as you type; Enter or Tab attaches
  the one you pick, the same as choosing it with +. It follows your
  project's .gitignore, so node_modules and build output stay out.

## 0.435.0 - 2026-09-28
<!-- big -->

### New

- **Hand-off chains.** A routine's steps can now go to different
  teammates: Wren finds the bug, Atlas plans the fix, Sable checks it. Each
  teammate is given what the step before them answered. Mark a step as the
  checker and the run only counts as done if it approves; if it asks for
  changes, the routine stops and says what they are. Set it up in a
  routine's editor: each step has "Who takes it" and "Checker". The card
  shows the chain, like "Wren → Atlas → Sable (checks)".

## 0.434.0 - 2026-09-28
<!-- big -->

### Improved

- **Locust is monochrome.** The lime accent is gone: buttons, switches,
  selected options, chips and the start-up screen are drawn in the same
  ivory as the rest of the app. Green stays only where it means something:
  a runtime that's ready, a step that's safe, a status light. Amber still
  means something needs you, and red that something went wrong. A teammate
  whose colour is lime keeps it.

## 0.433.0 - 2026-09-28

### Improved

- **About you keeps keys out, and only keys.** Your About-you note is
  read by every teammate on every provider, so a password or an API key in
  it is now refused, with a sentence saying why, whether you type it or a
  teammate suggests it. Personal details are welcome: names, places, family,
  how to reach you, how you like to work. Teammate memory already refused
  keys the same way.

## 0.432.0 - 2026-09-28

### Fixed

- **Long commands no longer squash the / menu.** A command with a long
  list of options pushed its description into a narrow column, one word per
  line. Every row is one line now: long options and descriptions end with
  "…", and hovering a row shows all of it. /auto-mode-setup, which changes
  Claude Code's own settings, is no longer offered.

## 0.431.0 - 2026-09-28

### Fixed

- **A Cursor teammate no longer changes your own Cursor's model.** Cursor
  saves whatever model a run asks for as your default, so after a Cursor
  teammate ran, your own Cursor opened on the teammate's model. Locust now
  puts your default back when the run ends. If you switched models yourself
  in the meantime, your choice stays. Only the model settings are touched.

## 0.430.0 - 2026-09-28

### Improved

- **"Working" is monochrome everywhere.** The dot on a working teammate's
  face, the dot beside "running" at the top of the window, and the Running
  marks on the Missions screen are now white, like the sidebar's spinner.
  Colour is kept for what needs you (amber) and what went wrong (red); the
  dot beside "runtimes connected" stays green.

## 0.429.0 - 2026-09-28

### Improved

- **The working marker in the sidebar is monochrome.** The little spinner
  beside a conversation that's running is now white like the rest of the
  sidebar, instead of the one green thing on it.
- **The / menu shows a runtime's own commands without scrolling to find
  them.** It's a little taller, so the section with Claude Code's, Codex's
  or OpenCode's commands starts in view under Locust's own.

## 0.428.0 - 2026-09-28

### New

- **Codex's commands, in the / menu.** With a Codex teammate, / now offers
  Codex's own /review, /compact and /init. /review reviews your current
  changes the way Codex's own review does (add what to focus on if you
  like), /compact summarizes the conversation so far, and /init writes an
  AGENTS.md for the project.

### Fixed

- **Claude Code's commands are in the / menu from the start.** Since
  0.426 they only appeared after your first Claude Code message following
  the update, so the menu looked like it had none. Locust now asks Claude
  Code for them when it opens, without sending anything or using your plan.

## 0.427.0 - 2026-09-28

### New

- **OpenCode's commands, in the / menu too.** With an OpenCode teammate,
  type / and you'll find OpenCode's own commands under its name: /init,
  /review, and your own commands and skills. Pick one, add anything it
  takes, and send: OpenCode runs it as that command, in Approve each as
  well, where it still stops and asks before it changes anything. The list
  is there from the moment Locust opens, no first run needed.

## 0.426.0 - 2026-09-28
<!-- big -->

### New

- **Claude Code's own commands, in the / menu.** Type / in the chat box
  with a Claude Code teammate and, under Locust's own commands, you'll find
  Claude Code's: /compact, /context, /init, /security-review and the
  rest, plus your own custom commands and skills, each with what it
  does. Pick one, add anything it takes, and press Enter: it goes to Claude
  Code as that command, exactly as if you'd typed it there. The list comes
  from Claude Code itself, so it stays current. Commands that would change
  your Claude Code setup (its model, settings, MCP servers, sign-in) are
  left out; you set those for a teammate in Locust.

## 0.425.0 - 2026-09-28
<!-- big -->

### New

- **Web pages open and run inside Locust.** When a teammate makes a web
  page, the conversation shows it working, not as code. Open it and it
  fills the viewer beside the conversation, with its scripts, styles and
  images, and a Source tab for the code. It runs in a frame of its own, so
  it can't reach Locust, your files or your accounts. A link it opens in a
  new window goes to your browser.

## 0.424.0 - 2026-09-28

### New

- **Teammates can suggest lines for About you.** When a teammate learns
  something lasting about how you work, it can suggest one sentence for
  your About-you note. The suggestion waits under "Waiting for you" on the
  Memory screen, with Add to About you and Dismiss. Nothing is added
  without you, and suggestions reach you even when teammate memory is off.

## 0.423.0 - 2026-09-28

### New

- **About you.** A note at the top of the Memory screen that every teammate
  reads before each run, on every runtime: how you like to work, what to
  always or never do. Only you change it, and it's given even when teammate
  memory is switched off. Edit it mid-conversation and the teammate is told
  the new note replaces the old one.

## 0.422.0 - 2026-09-28

### Fixed

- **Long lines in a change wrap instead of scrolling one by one.** In a
  change with long lines, like a new web page, each long line had its own
  little scrollbar and hid the end of its sentence. Long lines now wrap, so
  you can read the whole change without scrolling sideways.

## 0.421.0 - 2026-09-28

### Fixed

- **Notes between a teammate's steps show code properly.** When a teammate
  wrote a note with a block of code while working, the steps panel showed
  the raw ```` ``` ```` marks around it. Those notes now render like the
  reply does, including code blocks, bold text and links.

## 0.420.0 - 2026-09-28

### Fixed

- **The card after you stop a run no longer says it "did no work" after a
  long answer.** It said "Stopped before it did any work" even when the
  teammate had already written a lot. It meant no tools had been used, and
  now says so: "Stopped before it used any tools, so nothing is half-done."

## 0.419.0 - 2026-09-28

### Improved

- **Every Settings switch says what it controls.** Under Between teammates,
  the two switches were labelled only with what happens right now
  ("Messages wait for the recipient's next run."), so neither said what it
  was. They now read "Automatic replies: off. …" and "Urgent messages
  interrupt: off. …". Screen readers now hear each switch's name, not
  "Switch this on".
- **Switches that are off are easy to see.** An off switch was a dark knob
  on a dark track and almost disappeared. The knob is now a light grey.
- **Tidier wording.** Settings labels no longer end in full stops, and a
  typed "--" in several places is now a proper dash.

## 0.418.0 - 2026-09-28

### Fixed

- **Editing or switching off a memory now reaches a conversation already
  going.** A teammate you'd been talking to kept the old version: after a
  memory was changed and switched off, the teammate still quoted the old
  wording when asked again in the same conversation. It's now told that the
  new list replaces the old one.

## 0.417.0 - 2026-09-28

### Fixed

- **Save as routine names the model in plain words.** The dialog said who
  runs the routine "on OpenCode / opencode/nemotron-3-ultra-free", the
  model's internal id. It now says "OpenCode / Nemotron 3 Ultra Free", as
  the rest of the app does.

## 0.416.0 - 2026-09-27

### Improved

- **Room cards say who's in the room and when it was last used.** On the
  Rooms screen a room read "2 teammates · 1 post". It now names them, as in
  "Atlas · Quill · 1 post · 2m".

## 0.415.0 - 2026-09-27

### Improved

- **Missions lists conversations, the way the sidebar does.** Every message
  you sent had its own row, so "Good, ship it" sat alone meaning nothing,
  and the count didn't match the sidebar. Now each conversation is one row,
  named by how it started, with how many turns it took. Opening it opens the
  whole conversation, and deleting it deletes all of it.
- **Missions says when.** Each row shows how long ago it happened.
- **Longer titles on Missions.** In an ordinary-sized window the details now
  sit on a second line, so the title gets most of the row.

## 0.414.0 - 2026-09-27

### Improved

- **Search finds a conversation by anything you said in it.** It only looked
  at a conversation's name, which is its first message, so a word from a
  later message found nothing. It now looks at every message you sent, and
  typing a teammate's name finds their conversations.
- **Ctrl K finds conversations.** The palette now lists your newest
  conversations and searches all of them. In a narrow window, where the
  sidebar becomes a strip of faces and its search box is hidden, this is how
  you search.
- **Escape clears the search box.**

## 0.413.0 - 2026-09-27

### Fixed

- **Claude runs no longer stop when your account is near its limit.** Since
  0.407, a Claude run stopped in its first seconds with "the mission ledger
  could not be written" whenever Claude warned that your usage was getting
  high. The warning's two notes were saved in the wrong order, and Locust
  refused them. They are saved in order now, and the run carries on.

## 0.412.0 - 2026-09-27

### Fixed

- **No "install Node.js" note when you have Node.js.** Home could say
  "To use one in your own terminal too, install Node.js" on a machine that
  already had it, when Node was slow to answer while Locust was starting.
  Locust now looks for it where your own terminal does.

## 0.411.0 - 2026-09-27

### Improved

- **The model picker opens on the model you're using.** It always opened at
  the top of its list, so a teammate on a model further down meant
  scrolling to find it. It now opens with the chosen model in view.

## 0.410.0 - 2026-09-27

### Fixed

- **Approving a new file calls it new.** The approval card for a file that
  doesn't exist yet labelled it "MODIFIED"; it now says "ADDED", as the
  file does once it's written.
- **Saying no with a reason reads as declined.** When you denied an
  OpenCode teammate's action and typed why, the steps said "refused", the
  word Locust uses when a mode blocks something. They now say "declined",
  as they already did when you denied without a reason.
- **The approval card's change header sits inside the card.** Its first line
  touched both edges of the card; it's now inset like the rest.

## 0.409.0 - 2026-09-27

### Fixed

- **Making the window smaller keeps the newest reply in view.** If you were
  reading the end of a conversation and made the window smaller, the reply
  slid below the visible area. Locust now keeps you at the newest reply,
  unless you had scrolled up yourself.

## 0.408.0 - 2026-09-27

### Fixed

- **New teammate suggests a name you can use.** The name box always
  suggested "Wren", even with a Wren already on the team. It now suggests
  the first of its example names that isn't taken.
- **A teammate who hasn't run yet reads the same everywhere.** The Team
  screen said "route set by their first mission"; like Home, it now says
  "runs on the model you pick".
- **Sharing a team no longer mentions dropping the image.** Locust adds a
  team from an image with **Add team from image**; the Share window now
  says that.

## 0.407.0 - 2026-09-27

### Fixed

- **Claude's usage updates when it's high.** Once a usage window passes a
  certain level, Claude reports it as a warning, and Locust kept only the
  warning, so the usage card stopped updating exactly when the number
  mattered most. Warnings now update the card too, and readings already
  saved from earlier runs are picked up when you update.

## 0.406.0 - 2026-09-27

### Fixed

- **Usage says how old it is.** Locust knows an agent's usage only from its
  own runs, and the card never said when its reading was taken, so an old
  figure looked current. (Why Claude's figure stopped updating even after
  new runs is fixed in 0.407.) The card now says when the reading was
  taken, for example "From Locust's last run on it, Sat 21:40.
  Use outside Locust since then isn't counted." A usage window whose reset
  time has passed now says it has reset, instead of showing its old
  percentage. Codex's usage, read from your account, says so.
- **Updates say what happens when you restart.** Installing an update closes
  Locust for a minute or two and then opens it again by itself. The update
  prompt now says so before you click, and says it is installing after, so
  it's clear there is no need to open Locust again yourself. Opening it
  during the install would start the old version.

## 0.405.0 - 2026-09-27

### Fixed

- **Home fits beside a conversation.** With a conversation opened beside
  Home on a smaller window, Home is narrow, and several things in it were
  cut off. Now the team cards stack one per row instead of cutting off
  names and roles, the AI agent icons wrap instead of running under **Show
  all**, the chat box keeps its buttons inside it, and the Locust sign
  leaves out its tagline when there is no room for it.

## 0.404.0 - 2026-09-27

### Fixed

- **Keeping a routine's schedule no longer starts it again right away.**
  When a routine was held for review after its time had already passed,
  **Keep the schedule** made it due at once, and step 1 started within a
  minute, which the card only showed after the click. Keeping it now starts
  the routine's clock from the moment you decide, and the card says when
  the next run will be before you choose, for example "at Sun 4:30 PM, not
  straight away". To run it now, press its Run button.

## 0.403.0 - 2026-09-27

### Fixed

- **Home on a small window no longer looks cut off.** When Home is taller
  than the window, its top scrolls out of view, and the window's edge used
  to slice straight through the Locust sign. The top of Home now fades out
  while it is scrolled, so what is above dissolves instead.

## 0.402.0 - 2026-09-27

### Fixed

- **The Locust sign on Home is back to its full size when Home fits.** 0.401
  drew it smaller on Home screens that already fit the window, such as a
  team's Home at 1440x900. It now draws smaller only when Home is too tall
  for the window.

## 0.401.0 - 2026-09-27

### Fixed

- **The Locust sign on Home is no longer cut off for someone just
  starting.** With the team templates and the list of AI agents both on
  Home, a 1440x900 window was a little too short, and the teammates standing
  on the sign were cut off at the top. The sign now draws a little smaller
  when the window is short, so it shows whole, with room above it. On a
  smaller window it still scrolls away above the chat box, as before.

## 0.400.0 - 2026-09-27

### Fixed

- **A conversation cut off mid-work no longer shows a false answer.** When
  a teammate's run was interrupted (Locust closed under it, or it failed or
  was stopped), the last thing it said before its last step, such as "Let
  me check the file first", was shown below the steps as if it were the
  answer. It now sits inside the steps, where it was said, and the turn
  shows no answer, because it never gave one.

## 0.399.0 - 2026-09-27

### New

- **Build on an answer, merge the answers, read them side by side.** In a
  room, every finished answer now has **Build on**, which starts your next
  post to everyone as a reply to that answer. When a post has more than one
  finished answer, each also has **Merge**, which starts a post asking that
  teammate to combine them into one: what they agree on, where they differ,
  and whose each part was. Neither sends anything until you do. The new
  button beside More actions lays a post's answers out in columns so you
  can compare them.

## 0.398.0 - 2026-09-27

### New

- **Share your team as an image.** On the Team screen, **Share team**
  shows a card with every teammate's face, name, role and model, and saves
  it as a picture. The picture also carries the team, so anyone with Locust
  can add the same teammates with **Add team from image**, or with **Team
  from an image** on Home if they're just starting. Only names, roles, looks
  and models travel. Conversations, memories, folders, spending limits and
  models you added yourself stay on your machine, and a teammate set to Auto
  arrives on Edit.

## 0.397.0 - 2026-09-27

### New

- **Close the window, keep the work going.** When you close Locust while a
  teammate is working, the first choice is now **Keep working in the
  background**: the window closes, Locust stays in the system tray, and
  your teammates carry on, with a notification when they finish. The
  tray's tooltip says who's working. Click it to open Locust again, or use
  its menu to quit. **Quit anyway** still stops the runs, and **Cancel**
  leaves the window open.

## 0.396.0 - 2026-09-27

### New

- **Watch two teammates at once.** Right-click a conversation and choose
  **Open beside** to keep it open in a panel on the right while you work in
  another. It updates live as that teammate works. The chat box stays with
  the conversation in the middle, so there's never a question of where your
  message goes. Press **Open it here** to swap it into the middle and
  answer it.

## 0.395.0 - 2026-09-27

### New

- **Review notes stay with your message.** When you send notes you pinned
  to lines of a teammate's change, they no longer disappear into the end of
  your message. Each one is shown under it with its file, its line and what
  you wrote. Once the teammate has worked through them, each note says what
  happened: **Line changed**, **File changed, not this line**, **Not
  changed**, or **File deleted**. You can see at a glance which notes were
  dealt with.

## 0.394.0 - 2026-09-27

### Improved

- **A key is never remembered.** A teammate's memory goes into every
  teammate's instructions, whichever service runs them. Locust now refuses
  to keep a memory that contains an API key, access token, private key or
  password, and says so in the conversation. Edits to a memory and
  suggestions from a memory tidy-up are checked the same way.
- **Appearance reads like the rest of Settings.** Reply text size, Sidebar
  and Boot screen are one line each, with the choice on the right.

## 0.393.0 - 2026-09-27

### Improved

- **Settings, reorganized the way Claude's are.** The list down the side
  has an icon beside every page and groups them under small labels:
  Locust, Team, Agents, This folder and About. Pages that held several
  unrelated subjects are split up, so Memory, Between teammates, Your own
  models, Connectors and Privacy & data each have a page of their own.
  Settings opens on General.
- **Cleaner rows.** Settings no longer sit in boxes: each one is a row on a
  hairline, with its name on the left and its control on the right. Swarm,
  Auto mode and Plans are one line each with their switch, instead of
  saying the same thing twice. "How it works" is in plain type.

## 0.392.0 - 2026-09-27

### Improved

- **A new mark for a teammate at work.** A conversation that's running now
  shows Claude Code's own spinner in the sidebar, a small lime spark
  turning through ✢ ✳ ✶ ✻ ✽, in place of the dotted shape that morphed
  from circle to square. It stays sharp at sidebar size, and it becomes the
  quiet dot again when the work is done. With reduced motion turned on in
  Windows, it holds still.

### Fixed

- **A routine stopped on its last step has a way forward.** "Continue
  remaining steps" no longer sits greyed out when there's nothing left to
  continue: the card says it was the last step. A new **Keep the schedule**
  button clears the attempt and leaves the routine scheduled, so the next
  run comes when it's due instead of right away. Where an earlier step
  didn't finish, the card says why Continue waits.
- **An approval shows whose change it is.** An edit OpenCode asks to make
  is headed "as OpenCode would apply it", not "as Codex would".

## 0.391.0 - 2026-09-27

### New

- **What you do in the terminal comes back.** Open a Claude Code or Codex
  conversation in its own terminal with the `</>` button, keep working
  there, and come back: the conversation catches up when you return to
  Locust. Your prompts and the answers join the thread under
  "In Claude Code's terminal", and "Back in Locust" marks where Locust picks
  up again. Your next message carries on from there, in the same session,
  so the teammate knows what was said. An answer still being written stays
  in the terminal until it finishes. Other runtimes keep their sessions
  where Locust can't read them yet, and the button says so.

### Fixed

- **A routine waiting for your review is readable again.** On the Routines
  screen its card was squeezed into narrow columns, and the review box and
  the Continue and Abandon buttons were cut off. The card now spans the
  routine, reads top to bottom, and opens the saved conversation instead of
  printing its id.

## 0.390.0 - 2026-09-27

### Fixed

- **Codex's usage shows without running it first.** Locust now asks Codex
  for your usage when it reads Codex's models, so Codex's card on Home has
  its bars even if you haven't run Codex since updating. Nothing is spent
  asking.
- **Agent cards no longer cut off text.** The version now sits under the
  agent's name, so a long one like Cursor Agent's fits.

## 0.389.0 - 2026-09-27

### Improved

- **Point at an agent to see its usage.** The AI agents line on Home is
  plain logos again. Hover over a logo, or tab to it, and a card opens with
  the agent's name and version and a bar for each usage window its runs
  reported, with when each resets. Claude and Codex report usage.
- The model button keeps its amber dot from 80%, as before.

### Fixed

- **Long Antigravity answers no longer have a hole in the middle.**
  Antigravity cuts long answers short in the transcript Locust reads, which
  showed up as "1066 bytes the runtime did not keep". Locust now asks
  Antigravity for the whole answer before saving it. Answers saved before
  this update keep the note.

## 0.388.0 - 2026-09-27

### New

- **See how much of your plan each agent has used.** On Home, the AI agents
  line puts a thin ring around each agent's logo showing how full its
  current usage window is. The readings come from what that agent's own
  runs reported, so Claude and Codex have them. Hover a logo for the
  details, like "35% of the 5-hour window used, resets 22:10".
- The model button under the message box shows the same ring once that
  agent passes 80%, in amber, and red when the window is used up.

## 0.387.0 - 2026-09-27

### New

- **Open a conversation in its own terminal.** The new `</>` button in a
  conversation's header (and the same action in the ⋯ menu) opens that
  session in the agent's own terminal app: Claude Code, Codex, Copilot,
  Cursor, OpenCode or Muse Code. It opens in the teammate's folder, and in
  Windows Terminal when you have it.
  - Locust won't see what you do there, and the terminal runs with the
    agent's own permissions, not the conversation's mode.
  - It's available once a reply has finished.
  - Antigravity conversations stay in Antigravity's own window.

## 0.386.0 - 2026-09-27

### Improved

- **Home's teammate cards read alike.** Every card names its model the same
  way, so a Cursor teammate reads "Grok 4.7" like the others rather than
  carrying an effort level they don't. Hover a card's model line to see the
  full route, effort level included. The level is still set and shown beside
  the message box.

## 0.385.0 - 2026-09-26

### Improved

- **Codex shows the OpenAI logo.** Codex teammates now carry OpenAI's own
  logo everywhere Locust names the agent they run on.

## 0.384.0 - 2026-09-26

### Improved

- **Real logos for Gemini, Antigravity and Muse Code.** Gemini now shows
  Google's own sparkle, Antigravity its own arch, and Muse Code the Meta
  logo that Meta puts on it.
- **Home names the model each teammate runs.** A teammate on Claude's Opus
  reads "Opus 5.5" on Home, as it does on the Team screen, and Antigravity's
  Flash reads "Gemini 3.8 Flash". Each card shows the agent's logo in place
  of its name, so a long model name like "Grok 4.7 Medium" fits. Hover the
  line for the full route.

## 0.383.0 - 2026-09-26

### Improved

- **You can see which AI each teammate runs on.** Every AI agent now shows
  its logo, wherever Locust names it:
  - on your teammates' faces in the sidebar;
  - on their cards on Home and the Team screen;
  - in the model button under the message box, and in the model list;
  - in a conversation's header, and on approval cards.

  The model button's green dot is now the agent's logo, greyed out when that
  agent can't take work. The logos are built into Locust and never
  downloaded, so nothing outside your computer learns which tools you use.
- **Home shows your AI agents as logos.** When nothing needs you, the AI
  agents line on Home is a row of logos instead of a line of names. Hover
  one to see its name.

## 0.382.0 - 2026-09-26

### Improved

- **Closing Locust while a teammate works asks first.** Closing the window
  used to stop everyone's work on the spot. Now Locust tells you who is
  still working and asks: keep working, or quit anyway. Quitting still lets
  you resume from the conversation when you're back.

## 0.381.0 - 2026-09-26

### Improved

- **Friendlier to company laptops.** We checked how Locust starts the
  tools your teammates run against the patterns corporate security software
  flags. Your messages never pass through a command-line shell, and the few
  places Locust does start a shell now hand it a single, fixed line.

## 0.380.0 - 2026-09-26

### New

- **Your team, as a board.** When anyone on your team is busy, the Team
  screen sorts them into columns by what they need from you: **Needs you**
  (in amber: waiting on an approval or a question), **Working**, **Just
  finished** (in green, until you've looked), and **Ready**. Everyone at a
  glance, the most urgent first. When nobody's busy, it's the familiar
  roster.

### Fixed

- **A teammate's current job no longer shows as interrupted.** On the Team
  screen, a mission still running wore the red dot of an interrupted one.

## 0.379.0 - 2026-09-26

### New

- **Know when a teammate is done.** When a teammate you set going finishes
  a job that took a minute or more while you're in another app, Locust
  tells you, with the first line of what they said, and clicking it opens
  that conversation. A reply you were watching stays quiet, and teammates
  who finish together are told as one.
- **The taskbar shows when something needs you.** An amber dot sits on
  Locust's taskbar button while an approval or a question is waiting, and
  the button flashes when a new one arrives while you're elsewhere.
- **Your computer stays awake while a teammate works.** Windows won't go to
  sleep in the middle of a teammate's job; the screen can still turn off,
  and once nothing is running, your usual sleep settings apply again.

## 0.378.0 - 2026-09-26

### Fixed

- **OpenCode's Approve each works in your folder.** Opened the usual way,
  an OpenCode teammate in Approve each ran the commands you approved in
  Locust's own install folder rather than yours, while its card named
  yours. It now starts in your folder, as every other teammate does.
- **A Copilot teammate keeps to its own conversation.** Copilot has a tool
  that searches every Copilot conversation on your computer, and it ran it
  without asking. Locust now turns that tool off for its teammates.
- **A command waiting on you says "running", not "ran".** While an approval
  card was up, the line above it already said the command had run.
- **Copilot's model reads "Auto model".** Beside a teammate's mode, a bare
  "Auto" read like the Auto mode.

## 0.377.0 - 2026-09-26

### New

- **Copilot can ask before each action.** Choose Approve each for a Copilot
  teammate and it stops before every command and edit it makes, on the
  same card Codex and OpenCode use: approve it once, approve it for the
  rest of the run, or deny it and say why. Copilot reads your reason the
  moment the step it was on ends.

### Fixed

- **One thumb on the effort slider.** It had become two things at once: a
  crisp rectangle, with a gooey blob showing behind it whenever it moved.
  The thumb is now the liquid itself, in the same shape, so when you drag
  it, the thumb you are holding is what stretches and pours.

## 0.376.0 - 2026-09-26

### New

- **Notes on a teammate's changes.** Hover a line of a change and press
  its +, and your note sits under that line. Notes wait as one tile in the
  chat box, and your next message carries them all: each with its file,
  its line and the line itself, so your teammate reads exactly what you
  were looking at. Send them all at once, and your teammate isn't set off
  after every thought.

### Fixed

- **Your message keeps its line breaks.** A message you wrote over several
  lines showed as one run-on paragraph in its bubble. It now reads as you
  typed it, as a room's posts always have.

## 0.375.0 - 2026-09-26

### Fixed

- **A command you deny on Codex reads as refused, not failed.** Codex now
  reports a denied command as a failed script, so the row showed "failed"
  in red and the summary counted a command that "exited non-zero". Locust
  answered that approval itself, so it now says what happened: refused.

## 0.374.0 - 2026-09-26

### New

- **Deny, and say why.** Deny on an approval card now opens one line for
  your reason: "keep the folder, write kept.txt instead". Your teammate
  reads it and can change course instead of guessing, or trying the same
  thing another way. Press Enter with the line empty to deny without one.

## 0.373.0 - 2026-09-26

### New

- **What needs you, in one place.** When a teammate stops for your
  approval, ends a turn by asking you something, or suggests a memory, the
  title bar says so, "2 need you", beside what is running. Press it for the
  list: each line opens where it is answered, a paused run first, and the
  count goes down as you answer.

## 0.372.0 - 2026-09-26

### Improved

- **Keep all, for a tidy you agree with.** A tidy pass on a busy folder can
  leave ten suggestions waiting. "Keep all" answers them in one press, and
  anything kept can still be put back: a merge or a change shows what it
  was, and a forgotten memory waits seven days under Recently forgotten.
- **A suggestion no longer wears an on/off switch.** Waiting suggestions
  carried a kept memory's switch, drawn on but unusable. They now show the
  same spark as the memory card in the conversation.

### Fixed

- **Tidy up no longer loses a teammate's suggestions.** Asked to tidy
  memory, a teammate sometimes wrote its suggestions inside a code block,
  the way the request's own example showed them, and every one was
  dropped: you read "Here are my suggestions:" over an empty box, and
  nothing reached the Memory screen. Suggestions are now read wherever they
  are written, as a bulleted or numbered list too, and a line that still
  cannot be read is said in the conversation instead of vanishing. A tidy
  pass that puts nothing to you now says so under its reply.
- **What is waiting for you comes first on the Memory screen**, above how
  memory works: the place "answer it on the Memory screen" sends you.
- **"Suggested N changes, waiting below" goes once they are answered.** It
  stayed at the top of the Memory screen after every suggestion had been
  kept or turned down.

## 0.371.0 - 2026-09-26

### New

- **Ask one teammate in a room.** Press Reply on an answer, or type @ and
  a name in the room's box, and your next post goes to that teammate only —
  the others are not run. The teammate shows in the box before you send, the
  post says who it was put to, and the rest of the room is told it was not
  theirs. With no one named, everyone is asked, as before.

### Fixed

- **A room in the sidebar shows when it was last used.** A room you had
  just posted to could read "15h", the age of the room rather than of its
  last post. Replies teammates send each other now show their age there too.
- **No red dot for a teammate who was not asked.** In a post's header, a
  teammate the post never reached showed a red dot, as if they had failed.
  Nothing ran, so nothing failed: their face is now dimmed, with no dot.

## 0.370.0 - 2026-09-26
<!-- big -->

### New

- **A room remembers what was said.** Each post in a room used to reach
  every teammate fresh: not the earlier posts, not the other teammates'
  answers, not even their own. Now, before answering, each teammate is told
  the room's last few posts and the answers to them, so "Wren, say more
  about your second point" reaches a Wren who knows what that point was.
  Another teammate's answer is passed on as their own account, to check
  rather than take as fact. Your post is still exactly what you wrote.

## 0.369.0 - 2026-09-26

### Improved

- **One press off a busy free model.** When a free model's provider is
  turning requests away, the notice now names the next free model — "Stop
  and switch to Longcat 2.5 Preview Free". One press stops the waiting
  message, moves the chat box to that model and puts your message back, with
  its files. Press Enter to send it there.

### Fixed

- **The chat box stays on one line in a small window.** At the smallest
  window size, with a free model picked, the chat box's buttons split onto
  two lines. When space is short, the folder button now shows just its
  folder icon — the folder's name is in the title bar and in the button's
  hover — and the row stays on one line.
- **A busy model says so in Approve each too.** 0.368 told you when a free
  model's provider was turning requests away, but not after you had chosen
  Approve each action. Now it does, the same way.
- **"Starting…" stays true while a busy model waits.** Under that notice the
  line below read "Working…" though nothing was working yet. It says
  "Starting…" until the model answers.
- **Text boxes no longer light up in lime.** Clicking into a text box, or
  opening New teammate, which starts in its name box, drew a bright lime
  ring meant only for moving around with the keyboard. Text boxes now take
  the same quiet edge as the message box.

## 0.368.0 - 2026-09-26

### Improved

- **A busy free model says so.** Free OpenCode models are shared, and at busy
  times their providers turn requests away for a while. A teammate on one
  used to sit on "Starting…" for minutes with no reason given. Now the
  conversation says what the provider answered, that OpenCode keeps trying on
  its own, and that you can press Stop and pick another model to go on now.

### Fixed

- **A plan with steps left unticked no longer looks like a problem.** When a
  teammate finished its turn without ticking off every step of its plan — a
  writer asking you questions before it drafts, say — the plan showed
  "not checked off" in amber, the colour for something that went wrong. It
  is said plainly now, and amber is kept for a run that failed or was
  stopped.

## 0.367.0 - 2026-09-26

### Improved

- **Updates reach you without a restart.** Locust used to look for a new
  version only when it started. Now it looks again every six hours while it
  stays open, never while a teammate is working, and tells you the same way
  as before when one is ready.

## 0.366.0 - 2026-09-26

### Improved

- **Teammates working together read calmly.** When a teammate hands work to
  others, the lines that say how it is going ("Waiting on Sable, Penny",
  "Penny replied") are now quiet notes instead of amber warnings. Amber is
  kept for something that did not happen or needs you.
- A teammate answering on someone else's model now says so in plain words,
  without a model's internal name, and tells you where to give them their
  own. The strip above a conversation between teammates names models the
  way the chat bar does.

### Fixed

- A teammate's question could show its choices with stray brackets, such as
  "<option name>I'll supply my numbers". The choices now read as written.
- The whole window could slide up a few pixels, title bar and all, when
  something scrolled into view. It stays put now.

## 0.365.0 - 2026-09-26

### Fixed

- **Files a teammate makes are found in any folder.** Locust used to notice
  a file made by a command, such as a spreadsheet built with Python, only in
  a folder tracked with git. It now looks at an ordinary folder too, so the
  file is listed and opens beside the conversation wherever your teammates
  work. In a very large folder it says when it could not look at everything.

## 0.364.0 - 2026-09-26

### New

- **Spreadsheets open as spreadsheets.** A CSV, TSV or Excel file a teammate
  makes now opens beside the conversation as a grid: column letters, row
  numbers, a tab for each sheet, numbers lined up on the right. A new CSV
  also shows its first rows right in the conversation. Locust reads the
  cells and runs nothing; a formula the file holds no answer for is shown
  as written, and Excel or Sheets works it out.

### Fixed

- A file a teammate made by running a command, such as a budget workbook
  built with Python, is now listed as a changed file you can open, instead
  of not being listed at all.
- The Artifacts tab no longer says a reply "changed no files" when Locust
  could not see the folder to know. It says what it knows instead.

## 0.363.0 - 2026-09-26

### Improved

- **A new document shows as a document.** When a teammate writes a new
  brief, guide or draft, the conversation now shows it as a page, with its
  headings, lists and colour swatches, instead of a green block of raw text.
  Open reads the whole thing beside the conversation, and Show the change
  still gives the line-by-line view. A file that was edited still shows what
  changed.

## 0.362.0 - 2026-09-26

### Improved

- **Colours in a palette show their colour.** A hex code in a table, in a
  list or in bold text now has a swatch beside it, as it already did on its
  own in backticks. Bold or italic text holding code no longer shows the
  backticks as characters.
- Iris, the designer in the Write & design team, now suggests a one-page
  brand guide (palette, fonts and voice) instead of a web page. Locust never
  runs what a teammate writes, so a web page opened as its source code; a
  guide opens as a document you can read.

### Fixed

- The Artifacts tab said "A read-only mission produces none" under any reply
  that changed no files, even one that was allowed to. It now says what is
  true for that reply.

## 0.361.0 - 2026-09-26

### Improved

- **Your own model is called by its own name.** For a model you added under
  Your own models, the chat bar, the conversation header, team cards and
  every list that names a model now say "Acme Chat" instead of
  "OpenCode / Acme Chat". OpenCode still runs it; hover the chat bar to see
  the exact route.
- The panel that opens from Activity is now called "About this reply"
  instead of "Mission inspector".

## 0.360.0 - 2026-09-26

### Improved

- **Home fits a laptop screen.** With every AI agent ready, the list of them
  now folds to one line even before you have a team, so the first screen
  fits at 1440x900 and 1366x768 instead of scrolling. The three bots on the
  Locust logo are no longer cut off at the top.
- In a conversation's Activity panel, a plan's updates now say how far it
  had got ("Plan: 1 of 2 done") instead of repeating "Plan updated".

## 0.359.0 - 2026-09-26

### Improved

- **A team from Home speaks its own language.** Add the Research & money or
  Write & design team and each teammate now suggests first messages from its
  own world: Sable offers to compare where to keep savings, Penny to build a
  budget spreadsheet, Iris to design a one-page website. They used to offer
  questions about a codebase. Moss, the editor, is set up as an editor.
- Under a teammate's name, an empty conversation now says what their role is
  for, instead of "reads this workspace" for every role.

### Fixed

- **"What it may do" is now true for every AI agent.** The Activity panel
  told you each run was denied network access, even under an OpenCode run
  that had just searched the web. It now lists what Locust actually allowed
  that agent in that mode (changing files, running commands, searching the
  web, reaching outside the folder) and says anything else is left to the
  agent's own settings.

## 0.358.0 - 2026-09-26

### New

- **Chat only, for a model that cannot use tools.** Some models only chat:
  given the tools a teammate normally has, they refuse and the run ends.
  Set one of your own models to Chat only and its teammates talk with it —
  it just reads and changes no files. Test now checks this for you and sets
  it, and the model list says "chat only" beside it.

### Fixed

- An OpenCode run whose model refused a request said "OpenCode is trying
  again on its own" when it was not; the line now appears only once a retry
  is actually under way.

## 0.357.0 - 2026-09-26
<!-- big -->

### New

- **Add your own model.** A model your company runs, or one on this
  machine, can work on your team. In Settings, Runtimes, under Your own
  models, give it a name, its address and — if it needs one — a key.
  Anything that speaks the OpenAI chat API works: a company's own endpoint,
  vLLM, Ollama, LM Studio. Test checks the address serves the model before
  you add it. It then appears first in every teammate's model list, under
  Your models, by the name you gave it. The key is kept encrypted by
  Windows and is never shown again.

## 0.356.0 - 2026-09-26

### Improved

- **Calmer pages.** The sections inside Settings and Memory are named in a
  quiet line of their own — "How memory is kept", "Project folder" — rather
  than in large capitals. Each screen's title keeps the Locust lettering.

## 0.355.0 - 2026-09-26

### Improved

- **Activity says what happened in plain words.** "Ran npm test · done",
  "Changed signup.ts", "Searched the web for …", "Thought" — one line for
  each thing a teammate did, with how it ended. What a run may do is said
  in plain words too.
- **Routines, before you have any:** your finished conversations are one
  list, each with a "Save as routine" button.
- **The command palette** says "Back to the conversation".

## 0.354.0 - 2026-09-26

### New

- **Start with a team.** With nobody on your team yet, Home offers three
  ready-made teams — Build software, Research & money, and Write & design —
  of three teammates each, ready to talk to. They are ordinary teammates:
  rename them, change their faces or models, or remove them.

### Improved

- **New teammate** on an empty Home is the same button as the ones in the
  chat bar.

## 0.353.0 - 2026-09-26

### New

- **A monthly limit for each teammate.** In Edit teammate, set the most a
  teammate may spend in a month. Locust checks it before every run it starts
  for them — your messages, their replies to other teammates, routines and
  rooms — and once this month's priced runs reach it, that teammate starts
  nothing until you raise the limit or the month ends. A message you send
  stays in the box, with the reason. Runs on a plan or a free model are not
  priced, so they never count.

### Improved

- **A teammate's card says what they spent this month**, and against their
  limit when they have one.

### Fixed

- A teammate's cost, and the Missions total, left out older conversations:
  only your most recent ones were added up.

## 0.352.0 - 2026-09-26

### Improved

- **Teammates are handed the notes that matter.** Before a teammate starts,
  Locust pastes the saved notes that bear on what you asked. A word most of
  your notes share no longer makes every note look relevant, and rarer words
  count for more. When there is not room for them all, the newest notes are
  kept — before, the newest were the first to go, and the newest is usually
  the correction.
- **A teammate answering another teammate is handed notes about that
  message**, rather than notes matched to Locust's own instructions for
  replying, which are the same every time.

## 0.351.0 - 2026-09-26

### Improved

- **The Locust screen grows with your window.** On a tall window the
  machine and the three teammates on it draw larger instead of leaving
  the top of Home empty; on a small one they are as before.
- **New teammate is a proper button**, the same as the ones in the chat
  bar.
- **Your team is three across** at every window size.
- **Costs you see at a glance are money.** A conversation's header, a
  teammate's card and a relay between teammates show dollars or premium
  requests; token counts are in Details.
- **Home speaks to everyone:** "Your AI agents", not "coding agents".

## 0.350.0 - 2026-09-26
<!-- big -->

### Improved

- **Home leads with your team.** A card for each teammate — their face,
  what they do and the model they run on — opens a conversation with them.
  When every coding agent on this machine is ready, the list of them folds
  to one line; it opens again the moment one needs you.
- **Conversations read more like Claude's.** A conversation starts at the
  top of the window. The header shows the model, and says more only when
  something happened — running, stopped, failed. The record behind a
  finished conversation is one quiet "Details" link instead of a card.
- **Colour codes show their colour** beside them.
- **The sidebar's titles are longer**, and each row's actions button now
  appears when you point at the row.

### Fixed

- The receipt's text sat against the card's edge; Team cards did not line
  up when a model name was long; memory by-lines were larger than the
  memories; the Forget buttons were out of place; "All 1 hunk shown";
  Codex file rows said "file_change".
- Settings now says plainly that your teammates are the same in every
  folder, and what changes with the folder.

## 0.349.0 - 2026-09-26

### Fixed

- **A teammate's own branch is never handed over half-made.** On a big
  project, making a teammate's own copy could be cut off part-way and then
  used as it was -- to git, a copy with most of the project deleted. It now
  has the time it needs, and a copy left unfinished is finished before
  anyone works in it, keeping anything already there.
- **Removing the teammate you had picked** no longer sends your next
  message to a different teammate.
- **Stop on a run that had just finished** no longer leaves it stuck on
  "running".
- **A message you stopped while it was starting** is no longer sent later.
- **Typing with an input method** (Korean, Japanese, Chinese): Enter
  confirms the word instead of sending the half-typed message, and Escape
  no longer stops the run.
- **A run stopped by a teammate's message** says "This run was stopped",
  not "You stopped this run".
- **An opened activity list stays in its own conversation** instead of
  arriving open in the next one.
- **Copilot** no longer shows "Unhandled Copilot record" under its answers.
- **An Antigravity reply started by a teammate** is shown as theirs after
  a restart, not as a conversation you began.

## 0.348.0 - 2026-09-25

### Improved

- **A busy free model says so.** When the model's provider turns an
  OpenCode request away (a rate limit, most often on free models), the
  conversation now says what the provider answered and that OpenCode is
  trying again, instead of reading "Starting" until it gives up.

### Fixed

- **The check after a turn is still there when you reopen the
  conversation**, with its Send button.

## 0.347.0 - 2026-09-25

### New

- **Check after edits.** In Settings, give a folder a command that checks
  it (your tests, a type check, a linter). After a teammate's turn changes
  files there, Locust runs it and shows what is newly failing, or that it
  still fails the same way, or that it passed. Send to the teammate hands
  the failure back as their next turn; nothing is sent unless you press
  it. The command is kept by Locust for that folder, never read from the
  project.

### Fixed

- **A second edit to a file that was already changed is noticed.** Until
  now a teammate changing the same file twice in a row could look, to
  Locust, like no change at all.
- **Claude Code asking to run a command says so.** Its card spoke of "the
  service the connector reaches", as though the command were a
  connector's; it now names the command, and an edit names the file.

## 0.346.0 - 2026-09-25

### Fixed

- **An OpenCode edit waiting for your approval shows the change.** The
  card now shows the lines it would add and remove, and names the file
  within your folder, before you decide.
- **A declined action says "declined"**, not "failed" or "refused". A
  mode's refusal still says "refused".
- **OpenCode is no longer shown as "not signed in"** while it is still
  starting up; it needs no account. A coding agent that has not answered
  yet is checked again instead of being marked as signed out.
- **A teammate's question names its choices.** Some free models copied the
  instructions' example and titled their options "The first option" and
  "The second option".

## 0.345.0 - 2026-09-25

### New

- **Approve each action works on OpenCode.** Choose it and an OpenCode
  teammate stops before each command, edit or reach outside its folder,
  and shows you exactly what it wants to do. Approve it once, for the rest
  of the run, or decline it. Until now this mode was Codex only.

### Fixed

- **A command OpenCode was not allowed to run says it was refused**, instead
  of saying it ran and failed.

## 0.344.0 - 2026-09-25

### New

- **Ask two teammates on different coding agents for a review, at once.**
  When your team has reviewers on two different agents, a conversation's
  More actions menu offers both together, for example "Ask Booty (Claude
  Code) and Vale (Codex CLI) for a review". Each reviews in their own
  conversation, changes nothing, and opens with a verdict.

## 0.343.0 - 2026-09-25

### Fixed

- **A Codex teammate that starts a helper finishes its own answer.** The
  helper finishing used to end the teammate's run, with the helper's reply
  shown as the teammate's. Helpers now show as their own rows, and the
  teammate answers after them.
- **A continued Codex conversation shows what each message cost.** The
  count used to include every earlier message in the conversation.

## 0.342.0 - 2026-09-25

### New

- **LOCUST.md can speak to one coding agent.** Lines between `<claude>` and
  `</claude>` (or `<codex>`, `<opencode>`, `<cursor>`, `<copilot>`, `<muse>`,
  `<antigravity>`), each tag on its own line, go only to teammates on that
  agent. Everything else still goes to everyone.

### Improved

- **Muse Code's billing refusal says what it is.** When Meta wants the
  account's payment method verified, the card says so, and that it is
  settled in your Meta account, instead of an error code that looked like
  Locust breaking.
- **A LOCUST.md over the 200-line limit** now suggests moving long
  checklists into their own files and naming them; a teammate reads one
  when it applies.

## 0.341.0 - 2026-09-25

### Fixed

- **Opening a folder no longer runs a program its Git settings name.** A
  project's own Git configuration can name a program for Git to run while
  checking the folder, and Locust's own checks of your folder ran it. They
  no longer do.

## 0.340.0 - 2026-09-25

### Improved

- **A teammate on Claude takes an urgent message without being stopped.**
  When a teammate sends a Claude teammate something marked to take now, it
  reaches their run while it is going, and the work they were doing is not
  thrown away. It is still their next turn as well. Codex teammates already
  worked this way.

## 0.339.0 - 2026-09-25

### New

- **OpenCode models offer their effort levels.** Models that list levels,
  such as Ling 3.0 Flash Fin, Muse Spark and Space Bunny, now let you choose
  one where the composer used to say Fixed.

### Fixed

- **Auto mode on OpenCode can work outside the project folder**, which is
  what Auto is for. It was still being kept inside.
- **Antigravity shows each tool's result against the right call** when one
  step runs several at once. The first one used to read "did not report".
- **An OpenCode run that outgrew the model's context and recovered is shown
  as finished**, not as failed over the answer it gave.

## 0.338.0 - 2026-09-25

### Fixed

- **Connector permissions work on slower machines.** Locust gave up reading
  your connectors after 10 seconds, and on some machines listing them takes
  longer, so every connector call asked for permission.
- **Copilot's effort levels can be chosen.** The composer said Copilot's
  effort was fixed; its seven levels are offered now.
- **A tool call with an unusually long field name no longer stops the run.**
- **A downloaded update stays installable** after a later check fails.
- **A Windows "file in use" error during an install says so**, instead of
  suggesting you reconfigure npm.
- **Codex usage-limit warnings name their window**, such as "5-hour limit".
- **A run that could not keep up with its runtime's output says so**
  plainly, instead of blaming a large piece of output.
- **Antigravity runs are stopped cleanly when Locust closes**, and report
  their end when saving fails.
- **A connector check that failed is asked again next time**, rather than
  remembered as "no connectors" for five minutes.
- **Coding agents that update themselves are re-checked**, so Locust does
  not keep describing the old version.
- Smaller fixes to messages between teammates, permission handling and the
  release tooling.

## 0.337.0 - 2026-09-24

### Fixed

- **Locust opens from your taskbar as your installed copy, every time.**
  A development test could re-point the Start menu's Locust shortcut, which
  the taskbar goes through, at a build folder. That can no longer happen,
  and your installed copy puts the shortcut back if anything ever moves it.
- **A message queued with a file keeps its file**, and the file leaves the
  chat box with it instead of riding along on your next message.
- **Changing one setting no longer resets others.** Turning Auto on reset
  the layout, and other switches reset the send button's finish.
- **"All of Wren's conversations" shows Wren's**, from a teammate's card.
- **The Missions selection bar counts what Delete will delete.**
- **A refused room task keeps what you typed**, and says why.
- **A new file in the viewer opens on itself**, not on the last file's
  version.
- **A runtime that has not answered yet says so**, instead of asking you
  to sign in.
- **Headings like "Why C#" keep their last character.**
- **Rooms nobody has posted to are listed newest first.**
- **A routine saved from a message with a file is named after your
  words**, not the attachment line.
- **Replies cut off mid-word no longer lose their trailing s's.**
- **Non-English text in connector approvals stays intact.**
- **Stopping a run no longer flashes a console window** on Windows.
- **A Codex run that fails before it starts says why**, instead of "the
  runtime connection ended before the turn completed".
- **A Codex CLI that cannot start is reported at once**, instead of after
  a minute's wait.
- **Closing the window while a run is starting stops it from starting.**
- **Pruning old conversations deletes only what the preview listed**, and
  a restored one comes back to its teammate.
- **A crash while a teammate was sending a message no longer stops
  teammates messaging each other** until a file is fixed by hand.
- **An Update pressed while an automatic update runs installs once.**
- **Coding agents installed under a folder with accented letters** are
  recognised as in use, so they are not updated while running.
- **If Antigravity starts but Locust cannot record it**, you are told it
  is already working, so it is not started twice.

## 0.336.0 - 2026-09-24

### Fixed

- **A change you decline is shown as declined.** In Approve each action, a
  refused change was listed as a changed file, with the lines it would
  have written.
- **Diffs of files with `--` comments draw correctly.** Removing a SQL or
  Lua comment line split the file into a phantom second one, and an added
  line starting `++` renamed the file -- on approval cards too.
- **A teammate stops reading "working" when Claude Code's tools finish.**
  After a subagent or any tool, the face and sidebar stayed on "working"
  or "subagent working" for the rest of the run.
- **A read-only `sed` is a command, not an edited file.**
- **A reply that was only a file, a plan or a question is a reply.** It
  was told "This turn ended without a reply".
- **Rename works in the narrow sidebar**, from a teammate's card; New group
  is no longer offered there, where it named nothing.
- **Codex usage limits and sign-in problems are recognised** on the
  Approve-each transport, so another route is offered.
- **Long Antigravity answers keep their end**, where shares and notes are.
- **Connector permissions match local MCP servers with hyphens** in their
  names, so their tools stop asking every time.
- **Copilot replies that quote a key still finish**, so their shares and
  replies go through.
- **Muse no longer shows ready on an API key** that its runs are never
  given; sign in with `muse login`.
- **After OpenCode could not continue a session, the next message really
  starts a fresh one**, as the card says.
- **Accented and emoji text from Codex stays intact** in the record.

## 0.335.0 - 2026-09-24

### Fixed

- **Choosing another folder never closes Locust for good.** With an update
  downloaded, switching folders quit into a silent install that also shut
  the reopened app. The update now waits for your next quit, or Install.
- **The window fits a small screen.** On a 1080p laptop at 150% the window
  opened taller than the space above the taskbar, and could not be made
  smaller; the bottom of the chat box sat behind the taskbar.
- **After a handoff, the chat box stays on the runtime you handed to.** It
  went back to the one you had just left, and so did your next message.
- **Room posts and relayed replies run at the teammate's effort level.**
  They ran at the runtime's default whatever the teammate was set to.
- **Teammates whose names make the same branch name each get their own.**
  "Dev 1" and "Dev-1", or two names in a non-Latin script, collided, and
  the second could not start.
- **Edit runs start faster in folders with many untracked files**, and no
  longer read large untracked files into memory.
- **An attached file reaches a teammate on its own branch or in its own
  folder.** The message pointed at a path that did not exist where the
  teammate worked.
- **A scheduled routine runs only in the folder it was made in.** It used
  to run in whichever folder was open. Run still works anywhere.

## 0.334.0 - 2026-09-24

### Fixed

- **Handing a mission to another runtime keeps its mode.** A handoff or a
  resume in Plan, Auto or Approve-each ran as read-only Ask; a resumed plan
  lost its plan.
- **A handoff the other runtime would refuse stops nothing.** Picking Cursor
  for a read-only run on Windows, or a runtime that is signed out, stopped
  the running mission and then started nothing. The mission keeps running
  now, and the reason is shown above the chat box.
- **A teammate set to work on its own branch never runs in the shared
  folder.** When the branch could not be made, only a direct message was
  refused; a room post, a relayed reply, a routine or a resume ran in the
  main folder instead. They all refuse and say why now.

## 0.333.0 - 2026-09-24

### Fixed

- **A message that could not be sent says why.** Sending while another
  conversation without a teammate was running, or with every run slot
  taken, put your words back in the box with no reason; the reason is
  shown beside them now.
- **Resume from checkpoint says why it did not resume**, on the card you
  pressed, instead of seeming to do nothing.
- **A routine's Run says why it did not start**, on the Team and Routines
  screens, and a routine that does start opens its conversation.
- **Approval and question cards go away when their run ends.** A card left
  over from a stopped run kept its buttons and kept the teammate reading
  "waiting on you".
- **A follow-up typed while a teammate is busy elsewhere stays in the
  conversation you typed it in**, and is sent there when they are free. It
  could vanish, and later be sent into their other conversation.
- **Shift+Tab never turns Auto on.** Cycling through the modes could switch
  the workspace's Auto setting on as it passed.
- **A double-click never confirms a delete.** Deleting a conversation, or
  emptying the trash, asks first; a quick double-click used to answer for
  you.

## 0.332.0 - 2026-09-24

### Fixed

- **Menus stay open while a reply streams in.** Right-click menus and the
  chat box's model, effort and mode menus closed by themselves whenever the
  conversation scrolled to follow a reply.
- **Resuming an interrupted mission resumes it as its teammate** -- in their
  own branch or folder, and still theirs afterwards. It used to run in the
  main project folder, as nobody's.
- **Muse and Cursor work when your Windows user folder has a space in its
  name**, and arguments with characters like `&` reach them intact.
- **Two changed files with the same name are two files.** Changing
  `packages/a/package.json` and `packages/b/package.json` in one turn showed
  as one file, with one of the changes missing.
- **"Run it again" is offered only when nothing had started.** It could
  appear on a failed OpenCode, Muse or Antigravity turn that had already
  changed files, and pressing it would have done that work again.
- **Coding agents installed without Node.js now start.** On a machine
  without Node.js, an agent Locust installed could open a second copy of
  Locust instead of running.
- **A teammate who finishes while others wait for a turn keeps their board
  update.** Their task changes were dropped when every run slot was full.

## 0.331.0 - 2026-09-24

### Fixed

- **Older conversations open with their replies.** A conversation from
  before your last twenty or so missions opened showing only your own
  messages; its replies are read in when you open it.
- **Removing a teammate or a routine asks first.** The x on the Team screen
  and the routine Remove buttons acted on the first click. They now say
  what the second click will do ("Remove, with 2 routines?"), and a routine
  waiting for your review cannot be removed until you have dealt with it.
- **A program planted in a project folder is never run.** On Windows,
  Locust could run a `git.exe` saved inside the project you opened instead
  of the real git. It no longer looks in the project folder for programs.

## 0.330.0 - 2026-09-24

### Fixed

- **Removing a teammate's own branch no longer deletes their unsaved
  work.** Settings > Own branches > Remove used to delete every change a
  teammate had not committed, in one click and without a word. Now a copy
  with changes says which files would be lost and asks: Keep it, or Delete
  the changes and remove.

### Improved

- **Copilot's and Cursor's skills are listed** from the folders each of
  them reads.

## 0.329.0 - 2026-09-24

### Improved

- **Switching a conversation to another coding agent carries more of it.**
  The new agent is told the conversation's earlier turns, not only the
  last, and what the previous one did in plain words ("command: npm test")
  instead of internal ids.
- **A handed-over room task comes with a handover.** A teammate handing a
  task over is asked to tell the new owner what was done, how it was
  checked and what is left; if they say nothing, the new owner's board row
  says so.

## 0.328.0 - 2026-09-24

### Improved

- **A teammate's "I fixed it" comes with what Locust saw.** When a
  teammate messages another after changing files, the message now carries
  the files Locust itself saw that run change, so the one reading it can
  tell a claim from a fact.
- **Teammates hear about each other's edits.** When two teammates have
  both changed the same file in the last few hours, each one's next task
  starts with a note saying who changed it, so neither undoes the other.
- **An urgent message reaches a busy Codex teammate without stopping
  them.** A message marked to be taken now is shown to a Codex teammate at
  their next step, instead of waiting for their run to end or stopping it.

## 0.327.0 - 2026-09-24

### New

- **Set a teammate's effort from their Edit dialog.** Under their model,
  the same effort slider as the chat box, with that model's own levels.
  It used to change only in a chat with them.

### Fixed

- **Two teammates can no longer pass a room task back and forth.** A task
  handed to a teammate cannot be handed straight back to whoever handed it
  over; it stays with them, and the room says so. You can still move it
  anywhere yourself.

### Improved

- **The effort slider's thumb is Claude Code's.** It was a small, soft
  white ball; it is now a crisp white rounded rectangle the height of the
  track, with the filled part easier to see.
- **The line under the Locust screen on the home page is centred** under
  it, instead of starting at the column's edge.

## 0.326.0 - 2026-09-24

### Fixed

- **A failed update download is no longer "Locust hit a problem".** When
  an update could not be downloaded, Locust showed its crash message. It
  now says so quietly in Settings -- "The update could not be downloaded" --
  and tries again at its next check.
- **Two teammates can no longer share a name.** A message addressed to a
  name two teammates had reached neither; the New teammate and Edit
  dialogs now say the name is taken before you save.
- **A reply to a busy Antigravity teammate waits for them** instead of
  being dropped, as it already did for every other agent.

## 0.325.0 - 2026-09-24

### Fixed

- **A teammate started to answer a message is shown that message.** When
  older messages were already waiting, a teammate started to answer a new
  one could be shown only the older ones. The message it was started for
  now comes first.
- **The teammate you are waiting on is told you are there.** When a group
  question closed, or an answer was brought back, the teammate whose
  conversation you started was told nobody was in the exchange. It is now
  asked to tell you what came back, and may ask you something.
- **A run's memory is saved before the next teammate starts**, so they are
  briefed with it; and one step failing at a run's end no longer skips the
  steps after it.
- **A long brief keeps your project's LOCUST.md.** A run that belongs to no
  teammate dropped its whole brief when it ran long, instructions included;
  now the team memory gives way instead.

## 0.324.0 - 2026-09-24

### Fixed

- **A review of a long answer starts.** Asking a teammate to review a long
  piece of work was refused with "Enter a mission between 1 and 8,000
  characters". The review now fits: a long reply keeps its opening and its
  ending, and says how much of the middle was left out.

### Improved

- **A review opens with its verdict.** The reviewer starts with Ready,
  Needs changes or Start over, edits nothing, and checks the author's own
  claims ("the tests pass") rather than repeating them.
- **Settings shows the skills Codex and OpenCode actually load**, from
  their own folders -- including, for OpenCode, the Claude Code skills it
  picks up.
- **OpenCode's usage is complete.** Its reasoning tokens are counted in
  "out", its cached reads are kept, and a paid model's cost is shown.

## 0.323.0 - 2026-09-24

### Improved

- **An old message says it is old.** A teammate's message that waited half
  an hour or more before being read now says how long ago it was sent, and
  to check it still holds before acting on it.
- **The rules for replying are said once.** A teammate's second reply in an
  exchange gets one line reminding it of the rules instead of all of them
  again -- about 900 characters instead of 2,200 -- while still being told
  where it is in the limit.

## 0.322.0 - 2026-09-24

### Improved

- **A message that needs nothing back starts nothing.** When one teammate
  tells another that no further action is needed from them, and asks
  nothing, the message is waiting for them on their next run instead of
  starting a run just to read it. A message to the teammate whose
  conversation you are reading still starts their turn, as before.
- **The automatic-reply limit is counted right.** Two replies starting at
  the same moment can no longer both slip under the limit; a teammate is
  told its real place in the exchange ("automatic reply 5 of 12"); and all
  the runs one room post starts share a single limit, instead of one each.

## 0.321.0 - 2026-09-24

### Improved

- **A conversation is briefed once.** A teammate's standing instructions --
  its role, the team, the memory rules and the reply formats -- now go to
  the coding agent on the first turn of a conversation. Later turns carry
  one line saying they still hold, plus anything that changed. Measured on
  Claude Code and Codex: each later turn went from about 6,000 characters of
  instructions to about 350, so a long conversation lasts longer before the
  agent has to summarize it. The whole brief goes again after an agent
  summarizes its conversation, and every eighth turn regardless.
- **A summarized conversation says so.** When Claude Code, Codex or OpenCode
  summarizes a long conversation to fit the model, the thread shows one line
  saying so. OpenCode's summary, and its note to the model to carry on, no
  longer appear as the teammate's reply.
- **OpenCode in Ask and Plan mode keeps going when a shell command is
  refused**, instead of stopping there. A read-only OpenCode run no longer
  loads the project's own OpenCode plugins, and a new OpenCode conversation
  no longer spends a model call naming itself.

### Fixed

- **A turn that only messaged a teammate** no longer says "This turn ended
  without a reply" once a later turn follows it.
- **Claude Code runs no longer copy your command list.** Claude Code 2.1.281
  sends the list of your slash commands and skills at the start of every
  run; it is no longer saved into each mission's record.

## 0.320.0 - 2026-09-24

### Improved

- **An answer comes back to the teammate who asked.** When one teammate
  asks another a question and the answer is written in the other's own
  conversation without a reply, Locust now brings that answer back to the
  one who asked -- as a message from them -- and lets them carry on with it.
- **A message is delivered once.** The same message from the same turn can
  no longer reach a teammate twice.

## 0.319.0 - 2026-09-24

### Improved

- **See which memories nobody uses.** A memory no teammate has been given
  in a month -- and that nobody has changed -- is marked on the Memory
  screen, so you can tidy it or let it be. Nothing is deleted for it. The
  count starts with this version, so nothing is marked for the first month.

## 0.318.0 - 2026-09-24

### Improved

- **A memory that names a file says when the file has changed.** "Retries
  live in src/net.ts" is marked "may be out of date: src/net.ts changed
  since" once that file changes after the memory was written -- on the
  Memory screen and in what your teammates are told -- so they check before
  relying on it. A tidy pass looks at those first.

## 0.317.0 - 2026-09-24

### New

- **Tidy up memory.** On the Memory screen, Tidy up asks a teammate to read
  the folder's memories and suggest merging near-copies, retiring stale ones
  and correcting contradictions. Each suggestion waits for you -- Merge them,
  Forget it, Keep the change -- and nothing changes until you keep it. A
  suggestion about a memory that changed since it was made is refused.

### Fixed

- **"Ask X for a review" asks X.** The review ran as the teammate whose work
  it was, on their own route -- reviewing their own work. It now runs as the
  reviewer, in a new conversation of theirs, and your side of it reads
  "Review Wren's work." rather than the whole brief.

## 0.316.0 - 2026-09-24

### New

- **Recently forgotten.** Anything forgotten -- by you, by a teammate, or
  with Forget everything -- now waits under Recently forgotten on the Memory
  screen for 7 days, with Restore. A forget is no longer final the moment
  it happens.

### Fixed

- **A teammate in Ask mode on OpenCode finishes what it was asked.** Asked
  to look around a project, it would often try a shell command such as
  `git log`, which Ask mode does not allow, and the whole run stopped there.
  It is now told the shell is off and to read the files instead.
- **A changed memory names who changed it.** A memory a teammate rewrote --
  or that you edited -- was still credited to whoever first wrote it, on
  the Memory screen, in the conversation and in what teammates are told.
- **Memory switched off leaves nothing behind to read.** The memory file in
  your project folder kept listing everything remembered after you switched
  memory off; it now says memory is off.
- **A routine step too long to send says so.** A routine can no longer be
  saved with a step longer than a run can carry, and one saved by an older
  build stops before that step with the reason, instead of asking you to
  check for work that never started.

## 0.315.0 - 2026-09-24

### Improved

- **"Ask me first" now covers changes too.** With memory set to Ask me first,
  a teammate rewriting or forgetting something you kept no longer changes it
  on the spot. The change waits on the Memory screen beside what it would
  replace: Keep the change or Keep the old one, Forget it or Keep it. Until
  you answer, your teammates keep reading the memory as you left it.

## 0.314.0 - 2026-09-24

### Improved

- **See what a memory said before, and put it back.** On the Memory screen, a
  memory a teammate -- or you -- rewrote shows its previous wording, with Put
  it back; pressing it again undoes that too.
- **A memory rewritten today is today's.** Teammates see a rewritten memory
  as the newest, so the latest status wins over an older note, and the
  Memory screen says when it changed.

## 0.313.0 - 2026-09-24

### Fixed

- **A long room post still reaches every teammate.** A post near the length
  limit, plus the room's task board, used to be too long for any of them to
  start. The post now goes whole, and the board makes room, saying how many
  rows it left out.
- **A memory that could not be kept says so.** When team memory is full, or
  a line is too long, the teammate's thread now says it was not kept and
  why, instead of dropping it silently.
- **A room member who can never start leaves the queue.** Someone removed
  from the team, or an agent a room cannot post to, is recorded on the post
  with the reason instead of waiting there forever; a teammate who is only
  busy no longer holds up the rest.

## 0.312.0 - 2026-09-24

### Fixed

- **Team memory can no longer wipe itself.** If the memory file cannot be
  read -- damaged, locked for a moment, or written by a newer Locust --
  nothing is saved over it, and teammates are told memory could not be read
  this time. It used to be treated as empty, and the next thing remembered
  replaced everything.
- **An example is just an example.** When a teammate shows how a message,
  memory, task, question or file hand-off is written, inside a code block,
  Locust no longer acts on it.
- **What a teammate wrote is quoted, never obeyed.** Memories and the tasks
  on a room's board reach other teammates with their tags disarmed.
- **A reply no longer waits forever when every run slot is taken.** It
  starts as soon as any run ends, and the thread says that is what it is
  waiting for.

## 0.311.0 - 2026-09-24

### New

- **Set a teammate's model from their Edit dialog.** Right-click a teammate
  and choose Edit: the Model row names the model they run on, and Change
  opens the same picker as the chat box. Their messages, rooms and routines
  run on it. Picking another model in a chat with them still changes it
  too.

### Improved

- **The + menu matches the right-click menus.** New teammate, New room and
  New group, each with its key: T, R, G.
- **Settings › Updates says it in two words:** Beta builds.

## 0.310.0 - 2026-09-24

### Fixed

- **A conversation moved to another coding agent shows where it moved.**
  Pick a model on another agent partway through -- from Codex to OpenCode,
  say -- and the thread now marks the switch above your next message, the
  moment you send it. Opened again later, the conversation comes back whole
  and in order; it had been losing its first messages and showing the
  earlier answer under the later question.
- **The switch line names every agent.** It read "opencode" and the like for
  anything but Codex and Claude Code.
- **Saving a moved conversation as a routine keeps your words.** A step came
  out as the note Locust writes to brief the next agent.

### Improved

- **Settings › Updates says what its switch does.** Left off, Locust takes
  new versions as they are released; turned on, test builds too.

## 0.309.0 - 2026-09-23

### Improved

- **Your first teammate is one click from the home screen.** Until you have
  one, the home screen offers New teammate, and says what a teammate is:
  a name, a face and a model of its own. The New teammate form says it first
  too, rather than at its foot.
- **One word for each mode.** The mode menu now names Edit the way its
  chip does -- it said Accept edits -- and so do the messages that mention
  it.
- **The + menu no longer sits on the box's words.** While it is open, the
  box's placeholder steps aside.

### Fixed

- **A build newer than the day's no longer offers the older one.** With
  every build turned off again, Locust found the day's build -- older than
  the one running -- and offered it as an update that never came. Only a
  newer version is an update now.

## 0.308.0 - 2026-09-23

### Improved

- **The folder sits quietly on the composer again.** Its name shows as plain
  text beside the mode, the way it used to, rather than as a button; click
  it, or the + menu, to pick another.
- **Codex's notes about its own setup stay out of your conversations.** Its
  remarks on how it is configured -- a setting it does not recognise, skill
  descriptions it shortened -- came at the foot of every Codex turn. They
  are on Codex's row in Settings › Runtimes now, once.

## 0.307.0 - 2026-09-23

### Improved

- **A new profile starts on Claude Code or Codex when you have them.** It
  picks Claude Code if it is signed in, then Codex, and OpenCode's free
  models only when neither is. Any other is a pick in the model menu away.
- **One new build a day, or every build.** Settings › Updates has a switch:
  left off, Locust takes one new build a day, the one testers get; turned
  on, it takes every build as soon as it is out.
- **Settings says what Locust itself sends over the network.** Privacy &
  local data now names it all: Locust's own updates and new Codex CLI and
  Copilot CLI versions. The window makes no requests of its own.

## 0.306.0 - 2026-09-23

### New

- **Send feedback, the way Claude Code asks for it.** Settings › Report a
  problem, and every conversation's ⋯ menu, open a Send feedback box: say
  what happened, and Locust opens the report on GitHub with your Locust
  version and Windows build written in -- and, from a conversation, that
  conversation. You send it there (it needs a GitHub account). Nothing is
  attached for you: the log stays on your machine unless you add it.

### Improved

- **The folder is back on the composer.** The chip naming the folder your
  teammates work in is on the row again; click it to pick another.
- **Editing a queued message says what happens.** Edit takes the message
  off the queue and back into the box, and a line now says so -- and what
  Enter will do: queue it again while the run is going, or send it once the
  run is over.
- **The update banner waits for the run to end** instead of sitting between
  a live run and the box.

## 0.305.0 - 2026-09-23

### Improved

- **The home screen goes still when you leave it.** Its bots, the logo's
  flicker and the little loops around them ran for as long as it was open --
  roughly half of one processor core with the window in front, even with
  nobody there. Now all of it rests once nothing has touched the window for
  30 seconds, as the bots already did behind other windows, and wakes when
  you move the mouse or press a key. While it moves, each bot and each loop
  changes about 30 times a second rather than on every frame the screen
  draws.
- **Cleaner words under the machine:** one line where there were two
  sentences.

### Fixed

- **Presence dots and the waiting ring sit on the bot.** They were placed
  by the bot's box rather than by the bot, so on the home screen the dots sat
  at mid height and the blue bot's ring rode high over its antenna. They are
  placed from where each bot is actually drawn now: the dot on its lower-right
  edge, the ring round the whole of it.

## 0.304.0 - 2026-09-23

### Improved

- **Codex and Copilot update on their own again.** As in 0.302, Locust
  keeps them current by itself: it looks for a newer version a while after
  it opens, updates them when nothing is using them, and reads their models
  again. Settings › Runtimes still has the Update button, and its switch
  turns updating on their own off -- a choice Locust keeps.
- **Updates to Locust are small now.** Every new version used to download
  the whole installer, 117 MB, in the background. From the next update on,
  Locust downloads only the parts that changed -- about 2 MB for a typical
  release.

### Fixed

- **A Codex teammate's plan shows in the plan card.** Codex offers its
  planning tool only when it is asked to, and Locust never asked -- so a
  Codex teammate typed its to-do list into its reply instead, a "TODO" with
  "In progress:" and "Pending:" lines. Now its steps appear in the plan card
  and are ticked off as it works.

## 0.303.0 - 2026-09-23

### Fixed

- **Codex and Copilot update only when you ask.** 0.302 downloaded a newer
  Codex CLI by itself a minute after Locust opened -- about 160 MB, which
  can take over an ordinary internet connection for a minute or two, along
  with whatever else was using it. Locust still checks for new versions,
  quietly; a newer one is shown on its row in Settings › Runtimes with an
  Update button, and the download starts only when you press it. Updating
  on their own is a switch, off unless you turn it on.

## 0.302.0 - 2026-09-23

### New

- **Codex and Copilot stay current, and new models show up by themselves.**
  Locust reads each coding agent's models from the agent, so they appear as
  soon as the agent knows them -- but Codex CLI and Copilot CLI never update
  themselves. Now Locust looks for a newer version a while after it opens,
  never while it is starting, and updates them when nothing is using them;
  then it reads the models again. GPT-6-Sol and GPT-6-Luna come with Codex
  CLI 0.156.1. Settings › Runtimes says what it did, and has a switch to turn
  it off. Claude Code, OpenCode and Cursor Agent already keep themselves
  current.

## 0.301.0 - 2026-09-23

### Improved

- **The effort slider moves with you.** Drag it and the thumb stays under
  your pointer with the filled part beside it, the level changing as you
  pass each stop; let go and it settles on the nearest one. It used to jump
  from stop to stop behind the pointer, with the fill catching up on its
  own. And the liquid look it was built with now shows: the thumb stretches
  into a drop as it moves and pours onto its stop.

## 0.300.0 - 2026-09-23

### New

- **Teammates notice things.** A teammate's face in the sidebar hops once
  when its turn ends, so you can see who just finished without reading
  anything. When one teammate hands another a message, their two faces
  turn and look at each other for a moment.
- **The title screen notices you.** The sleeping Hopper opens its eyes when
  your pointer comes near and watches it, then dozes off again once you
  have gone. A click on the screen powers the logo on again, and turning
  swarm on sends a few small swarm bots up across it.

### Improved

- **A queued message looks like the message it will be.** A message
  waiting to send sits over the box as your own message, outlined until it
  goes, with one line under it: when it sends, then Edit and Discard.

### Fixed

- **A queued message stays with its own conversation.** A message queued
  for one teammate showed under every other conversation too, and Edit or
  Discard there changed or removed it. Two messages queued for two
  teammates could even be joined into one and sent to one of them. Each
  conversation now shows, edits and sends only its own.

## 0.299.0 - 2026-09-23

### Improved

- **What a teammate says while it works stays where it said it.** Some
  models narrate as they go — "Creating your file…", "File write is
  underway…" — and Locust drew all of it after the finished work, so it
  read as stale. Once a turn is done, those lines now sit in the fold
  among the steps they came before, the way Claude Code keeps them, and
  only the answer is below. Nothing is dropped; it is back in the order
  it was said.
- **One style for small labels.** The small capital-letter labels across
  the app — section heads, field names, the code language on a code block,
  the model picker's groups — now share one size and spacing in the code
  font. A few were a size off, and three had no working size at all. Room
  names on the Rooms screen now match teammate names on the Team screen.

### Fixed

- **Click anywhere in the chat box to start typing.** Only the one line of
  text took a click; the rest of the box did nothing. Now the whole box
  puts the cursor in the message, and its buttons still work as before.

## 0.298.0 - 2026-09-23

### Fixed

- **A message you queue is sent when the turn in front of it ends.** If you
  wrote the next message while a teammate was still working, it could
  vanish when that turn finished: Locust sent it a moment before it had
  finished tidying up after the turn, was told the teammate was still busy,
  and dropped it — leaving you on the teammate's home screen, with your next
  message starting a new conversation. The next turn now goes straight
  through, and a message that still can't go yet waits in the queue and
  tries again instead of disappearing.

## 0.297.0 - 2026-09-23

### Improved

- **Less work at the end of every turn.** Each time a turn finished, Locust
  sent its window the newest twenty conversations again in full — about
  3 MB on a busy history, nearly all of it what the window already had —
  and the window rebuilt every earlier turn on screen from it. It now sends
  only what changed: about 0.16 MB on the same history, in roughly half the
  time, and the turns you are reading are left as they are.

## 0.296.0 - 2026-09-23

### Improved

- **A face says what its teammate is doing, in words.** The faces along
  the top of the sidebar, and the faces in the narrow sidebar of a small
  window, showed a teammate working or waiting on you only by moving, a dot
  and a colour. A screen reader now hears it too: "working", "waiting on
  you", or why the teammate is blocked.

### Fixed

- **Arrows and signs in a reply are drawn as themselves.** A teammate that
  wrote `$\rightarrow$` for an arrow showed exactly that. A lone symbol
  written that way — an arrow, ×, ≤, ≠, ∞, a Greek letter — is now drawn
  as the symbol; sums of money and whole formulas stay as written.
- **Locust's own teammates draw at once.** The Hopper and the Swarm waited
  for the screen's next frame before drawing, so where none came — a
  window or tab not yet shown — they were blank beside teammates of every
  other shape.

## 0.295.0 - 2026-09-23
<!-- big -->

### New

- **The title screen is a machine.** The home screen's logo and name now
  sit on a small screen of their own — the same screen the loading
  display uses — with "Autonomous teammates on your own machine" on the
  glass under them, and the three teammates standing on top of it. The logo
  powers on inside the screen. It also fits the smallest window without
  scrolling.

### Improved

- **The usage warning is said once a conversation.** "You've used 64% of
  your 7-day window" appeared on every turn. It now appears once in each
  conversation; a limit actually reached still shows every time.
- **A long turn keeps its answer in view.** A finished turn's work stays
  open, as before, but past a dozen steps it shows the first ten and "Show
  N more", so the answer is not pushed off the screen.
- **A routine says which step it is on.** While a routine runs, its
  conversation shows "step 2 of 3" in the sidebar and names the routine and
  step at the top of the conversation.
- **A subagent at work shows in the sidebar.** While a teammate has a
  subagent working, that conversation's row says "subagent working".
- **A room you have not posted to yet stays in the sidebar**, so a room you
  make and leave is still one click away.

## 0.294.0 - 2026-09-23

### Improved

- **Settings › Runtimes fits on one screen.** Each CLI's own agents and
  commands were listed in full under it, so the page was a long scroll of
  other programs' settings. They are folded under a line that counts them
  — "3 agents · 3 commands set up in this CLI" — and open with a press.

- **Cursor's models say their levels in the picker.** Every Cursor row read
  "Listed by cursor-agent --list-models". Each now says what it offers in
  words, like "Low to Max · Fast".

## 0.293.0 - 2026-09-23

### Improved

- **Small teammates have smooth edges.** Bots in the sidebar, the
  conversation and the lists were drawn with hard, jagged outlines at their
  small sizes. They are drawn at twice the size and shrunk, so their edges
  are smooth, at no measurable cost.

- **Shorter Settings names.** The pages are Workspace, Runtimes, Teammates,
  Appearance, General and Changelog. "The send button" and "The boot screen"
  lose their "The", and the Workspace section about teammates replying to
  each other is "Between teammates".

- **A new teammate starts on a new colour.** Every new teammate started on
  lime, so a team made with the defaults was all one colour. The colour
  picker now starts on the first one nobody on the team wears yet.

### Fixed

- **A turn's reads name their files.** A model that reports the whole path
  of each file it read showed "read 3 — C:\Users\…" cut off at the edge of
  the row. The row names the folder and the files within it.

## 0.292.0 - 2026-09-23

### Improved

- **A routine's run is marked in the sidebar.** A routine replays a
  conversation's words, so its run had the same title as the conversation it
  came from, and the two could only be told apart by opening them. Its row
  carries the Routines clock now, and pointing at any row names the teammate
  and, for a routine's run, the routine.

- **The inspector speaks in words.** Activity listed the record's own event
  names — `runtime.started · codex`, `tool.completed · shell`,
  `run.completed`. It now reads "Started on Codex CLI", "shell · wc -l
  README.md", "shell finished · exit code 0", "Finished". Details names the
  runtime and model the way the composer does and says which sign-in the run
  used, with the exact strings still there when you point at them.

- **Appearance puts reading first.** Reply text size is at the top of the
  page, above the sidebar and the send button's effects.

- **The update banner matches the app.** A card with the install in the
  app's offer colour, not a navy strip.

- **A cost column never shows a bare dash.** A run that reported no cost
  says "not reported", and a free model's run says "free".

- **Teammates trust a file over a note copied from it.** A remembered value
  that came from a file in the folder is the file as it read then; when the
  file matters to the work, the teammate reads the file.

## 0.291.0 - 2026-09-23

### Improved

- **Cursor's models read as names.** The picker listed many of Cursor's
  models by their ids — `claude-opus-5-5`, `cursor-grok-4.6` — and gave
  their Max, None and Minimal levels rows of their own, so Kimi K3 appeared
  twice. Each model is now one row under the name Cursor gives it ("Claude
  Opus 5.5 1M"), with every level on its effort slider: 49 rows where there
  were 78. When the level you were on is not one the new model offers, it
  starts on the level Cursor itself uses by default — Max for Kimi K3.

- **A run has one clock.** The Missions row timed a run from when it was
  created to its last write, so it disagreed with the conversation's own
  line (10s against 6s). Both use the run's start and finish now, and the
  row no longer opens with "0 checkpoints".

### Fixed

- **Save as routine proposes only the turns that worked.** A turn you had
  stopped was offered as a step.

- **A tool call written as text shows as code.** Some free models write the
  call they meant to make instead of making it, and the thread drew it as
  the first paragraph of the reply. It is a code block labelled "tool call"
  now, and what the model said to you stays as words.

- **A Claude version reads as a version.** A Cursor run on Claude Opus 5.5
  read "Claude Opus 5 5 Medium" in its row.

## 0.290.0 - 2026-09-23

### Fixed

- **Everything a Claude teammate says in a turn stays in the thread.** Each
  new message replaced the one before it, so a turn that spoke three times
  around its work showed only its last sentence. They are all kept now, in
  order.

- **A subagent's words are never shown as the teammate's.** A helper's report
  could take the teammate's place in the conversation, or have the
  teammate's next words run on after it.

- **A command Claude Code refused says "refused".** It read as failed, and
  the turn's line counted it as a command that ran and exited with an error.
  It never ran; the row says so, with Claude Code's reason when you point at
  it.

## 0.289.0 - 2026-09-23

### Improved

- **A long command says what it is for while it runs.** Under a Claude Code
  teammate the live line read "Using a tool... Bash" for as long as a command
  ran. It now says what the teammate said the command is for — "Run the test
  suite" — within a second of it starting.

### Fixed

- **"Free" stays on the model chip.** A free model's name was cut just before
  the word Free. It is a tag of its own now, and never cut.

- **Settings say why.** The relay's own settings are greyed out until
  teammates reply to each other, and now say so when you point at them.

## 0.288.0 - 2026-09-23

### Fixed

- **A room stays in the sidebar.** A post to a room left one row per teammate,
  each titled with the post, and no row for the room itself. The room now has
  one row, by name, where its newest answer is; pressing it opens the room.

- **The context gauge no longer looks like it is loading.** It is a small pie,
  filled as far as the conversation's context is. The arc it replaces read as
  a spinner still going after a run had finished.

- **A turn's line counts what it shows.** "3 tool calls · ran mkdir, printf and
  10 more" over twelve commands now reads "ran 12 commands: mkdir and printf ·
  3 other tool calls", and a command is named past the `cd` in front of it.

- **Said once.** Codex's note about its skills budget shows on the first turn
  that hears it, not on every turn, and a run shows one usage warning, its
  latest, instead of one each time the figure moves.

## 0.287.0 - 2026-09-23

### Fixed

- **A long conversation keeps up with its newest reply.** From the second or
  third turn, a reply could end below the bottom of the window with the
  "newest message" arrow up, though nobody had scrolled. It stays with the
  answer now; scrolling up still stops it.

- **A turn's plan shows that turn's steps.** OpenCode keeps one to-do list for
  a whole conversation, so every turn's plan listed the steps of every turn
  before it. Each turn shows its own.

- **Windows High Contrast.** The Settings switches, the chosen reply cap, the
  current Settings page, the status dots and the focus on the search box had
  all disappeared in High Contrast. They show again, in the system's colours.

## 0.286.0 - 2026-09-23

### Fixed

- **Message anyone on your team.** With six or more teammates the sidebar
  shows four faces and a count, and the Team screen that count opens had no
  way to start a conversation with the others. Every card on the Team screen
  now has a Message button.

## 0.285.0 - 2026-09-23

### Fixed

- **The context ring reads what the conversation holds.** On a long Claude
  run it added up every step, so a conversation holding 240k of a 1M window
  read "5M of 1M". It now reads the conversation after the run's last step.
  Conversations from before this build show the ring again after their next
  reply.

- **No cost on a subscription.** A Claude run your plan covered shows "in
  your plan" instead of a dollar figure, and the ring shows only the context.
  Runs billed per token still show what they cost.

- **Usage warnings in words.** "seven_day limit allowed_warning · resets
  2026-09-28T07:00:00.000Z" now reads "You've used 53% of your 7-day window ·
  resets Mon 03:00".

## 0.284.0 - 2026-09-23

### New

- **Effort, the way Claude Code shows it.** The effort control reads "Effort
  High" with a ? that says what the level costs, runs from Faster to Smarter
  over a dotted track, and its white thumb pours from one level to the next.
  Levels read as words: Low, Medium, High, Extra high, Max. A model with fast
  variants keeps its Fast switch.

### Improved

- **One font in the message box.** The model's name and the effort level are
  in the same typeface as the rest of the row.

- **"Fixed".** A model that sets its own effort says Fixed, not "effort ·
  fixed".

## 0.283.0 - 2026-09-23

### New

- **A new message box.** The box you type in is now one rounded box with its
  controls inside it: round buttons, frosted chips, and a metal Send. The
  permission mode sits beside the +, and the model and its effort sit
  together on the right.

- **The + opens up.** Press + and it splits into two: Attach files, and
  Choose a folder. The folder chip is off the row, since the folder's name is
  in the title bar, and the row still warns you when no folder is chosen.

### Improved

- **Shorter mode names.** The chip reads Ask, Edit, Plan, Approve or Auto,
  the same words the / commands use. The menu still gives each one's full
  name and what it allows.

- **Swarm is a command.** The Swarm button is gone from the message box.
  Type /swarm, or turn it on in Settings, and the effort chip shows the
  locust mark while it is on.

### Fixed

- **Centred buttons.** The +, Send and Stop glyphs sit exactly in the middle
  of their buttons.

## 0.282.0 - 2026-09-23

### Improved

- **A clearer Stop button.** While a teammate works, Stop is now a round
  button with a small square in the middle, and the grey light that travels
  its edge is easier to see.

### Fixed

- **Centred buttons.** The arrow on Send and the square on Stop sat half a
  pixel right of centre and a little low. Both are in the middle now.

## 0.281.0 - 2026-09-23

### New

- **What's new, in Settings.** Every version of Locust and what it changed,
  newest first, the way Claude Code shows its own: the date, the version, and
  the changes grouped as New, Improved and Fixed. Settings, then What's new.

- **A splash for big updates.** When an update changes something you need to
  know about, the home screen tells you once, the first time you open it.
  Everything else waits in What's new.

### Improved

- **No banner on the home screen.** "Locust 0.277.0 is running. Here is what
  changed." is gone, and so is the scrollbar it added to the title screen.

## 0.280.0 - 2026-09-23

### New

- **Antigravity's questions reach you.** When a teammate on Antigravity asks
  you to choose between options, the question now appears in the
  conversation as a card with each option, a box for your own answer, and
  Skip. The teammate shows as waiting on you in the sidebar and the top bar,
  and you get a notification if you are in another window. Answer it in
  Locust and Antigravity carries on with your choice. Answer it in
  Antigravity's own window instead and the card goes away by itself. Before
  this, the question only showed inside the folded tool calls, and the
  teammate sat waiting on an answer nobody could see.

### Fixed

- **Answers on question cards arrive.** Fixed answers given on any question
  card being lost on the way to the teammate, so it was told you said
  nothing.

- **No "Using a tool" line above a question.** While a card waits on you,
  the line above it no longer says the teammate is busy with a tool.

## 0.279.0 - 2026-09-23

### Improved

- **Calmer bots, except the one you are talking to.** The bot beside a
  conversation's live line keeps all its moves: it hops while it works and
  flips now and then. Its copies in the sidebar and the top bar no longer
  mirror every hop. There, a working teammate looks around and gives a
  small bounce, so you can still tell who is busy without the whole window
  moving at once.

- **The title beam means loading.** The grey beam round the home screen's
  title box now runs only while Locust is finding your coding agents, and
  goes out as the logo lights and the bots wake. It no longer circles
  forever beside them, which made the home screen look like it was still
  loading when it was not, and the home screen does a little less work.

## 0.278.0 - 2026-09-23

- **A quiet beam of light.** While a run is going, a soft grey light now
  travels round the Stop button, so you can see at a glance that something
  is working and where to stop it. It leaves when the run ends. And the
  title box on the home screen has one too, a slow grey beam running round
  its edge once your coding agents have been found.

- **The home screen rests when you are not looking.** The bots and the
  beam on the home screen stop moving while Locust is in the background and
  pick up again when you come back, so a home screen left open behind your
  other work no longer keeps your computer busy.

## 0.277.0 - 2026-09-22
<!-- big -->

- **Every teammate is a bot.** The pixel faces are gone. In the sidebar, the
  conversation, the Team screen, rooms and routines, every teammate is now
  one of twenty little bots: the library's eighteen, plus Locust's own
  Hopper and Swarm. Your teammates each already have one, drawn from the
  look they had, so nobody turns into a stranger. They keep still while
  they are idle, hop while they work, and look around while they think or
  wait on you, still wearing the amber ring and dot when it is you they are
  waiting for. When Locust itself speaks, it is the Swarm.

- **Choose a look.** New teammate and Edit teammate now have a **Look**
  grid: pick any of the twenty shapes, eyes alone or a mouth, or Shuffle
  for a surprise. And five new colours sit beside the four: teal, butter,
  rose, slate and pearl.

## 0.276.0 - 2026-09-22

- **New teammate fits the window again.** On a shorter window the New
  teammate form ran off the top and the bottom of the window, taking its
  title and its **Create teammate** button with it. Every dialog now stays
  inside the window and below its title bar: the title and the buttons stay
  where they are, and the form between them scrolls. Save as routine and
  Group settings get the same.

## 0.275.0 - 2026-09-22

- **Older Claude versions, folded away.** Under Claude Code's models in the
  picker there is now one quiet row, **Older versions**, that opens eight
  earlier models in place: Opus 5, 4.8, 4.7, 4.6 and 4.5, Fable 5, and
  Sonnet 4.6 and 4.5. Each stays on exactly that version, where the models
  above it always move to the newest. Searching finds them without opening
  the fold, and the one you are using is never folded out of sight. The chip
  names it too: *Claude / Opus 4.8*.

- **A calmer home screen.** The ghost is white now and floats instead of
  hopping and spinning, and the Locust bot is green.

## 0.274.0 - 2026-09-22

- **Bots on the home screen.** The three teammates on the home screen are
  now bots: a ghost hard at work, a droid waiting on you (in Locust's amber
  ring), and a Locust of our own, the Hopper, with big folded grasshopper
  legs, fast asleep. They follow your pointer when it comes near and hop
  when you click them, and like before they stay still until your coding
  agents have been found. Teammates everywhere else keep their pixel faces
  for now.

## 0.273.0 - 2026-09-22

- **Claude's models say which version they are.** The picker listed Claude
  Code's models as Opus, Fable, Sonnet and Haiku, each "the newest", without
  saying which. They now read **Opus 5.5, Fable 5.1, Sonnet 5 and Haiku
  4.5**, and the chip under the message box reads *Claude / Opus 5.5*. The
  versions come from Claude Code's own table of what each name means; they
  still move with the family, and once you have run one, it shows exactly
  what that run used.

- **No more "Unhandled Claude record" in the middle of a conversation.** A
  new kind of message from Claude Code, sent while a long tool is still
  running, showed up as a line of its own reading *Unhandled Claude record:
  tool_progress*. Locust now understands it and says nothing. And the next
  time any coding agent sends something Locust has not learned yet, the note
  goes into the run's folded work details instead of your conversation.

## 0.272.0 - 2026-09-22

- **The home screen is the whole cover now.** 0.270 put only the Locust
  logo there. The card now carries all of the design system's cover: the
  logo, with *Autonomous teammates on your own machine* centred under it in
  the app's mono capitals, and three teammates on a plate beside it — one
  working, one waiting on you, one idle. They sit still until your coding
  agents have been found, then come on with the logo, which still lights up
  after the loading screen and again every 7 to 10 seconds. The name is set
  in the app's own Figtree, bolder than the old traced lettering, exactly as
  the cover sets it, and the whole card is drawn to fit your window's width.

## 0.271.0 - 2026-09-22

- **The working orbs are sharper.** The six busy orbs beside a working
  teammate (the ribbon, the web, the braid, the globe, the wave and the cube)
  were drawn at 64 pixels and squeezed into their 26-pixel spot by the
  browser on every frame, which striped some of them and made their dots
  flicker as they moved. They are now drawn four times larger by the same
  library and shrunk with a high-quality filter: the same designs, drawn
  true, and steady from one frame to the next. They look a little dimmer,
  about 15%, because the old extra brightness was dots doubling up. The ring
  and the square were already sharp and are unchanged.

## 0.270.0 - 2026-09-22

- **The logo on the home screen powers on.** Once your coding agents have
  been found and the app opens, the Locust mark and wordmark light up like
  the loading screen's tube: a flicker of phosphor lime, one sweep down the
  glass, then they cool to white. It lights again every 7 to 10 seconds, in
  case you missed it. Settings › Appearance › **The boot screen** decides it
  too: Full lights it and relights it, Subtle lights it once, Off keeps it
  still. It never moves if your system asks for reduced motion.

- **The Receipt tab has the receipt as soon as a run finishes.** It used to
  say the receipt would appear "once this mission has been recovered from
  the ledger", with nothing to press, right after a run whose record was
  already saved. It now shows the run's phase, checkpoints, event count and
  whether its record verifies.

## 0.269.0 - 2026-09-22

- **The right-click menu stays in the window, and reads like Claude's.**
  Right-clicking a conversation low in the sidebar opened a menu that ran
  off the bottom of the window. It now opens upward, or to the left, when it
  would not fit, and so do the lists it opens. It is shorter and quieter too:
  no title bar, a thin line between kinds of action, and **Assign to** is one
  row that opens your whole roster, where it used to be a row per teammate.
  A letter at the end of a row is a real key while the menu is open: R
  renames, D deletes (after asking), and the arrow keys move through it. The
  teammate and group menus got the same treatment.

## 0.268.0 - 2026-09-22

- **A new typeface.** Everything Locust says is now set in **Figtree**, in
  place of Geist: the sidebar, buttons, settings, cards and your own
  messages. Screen titles (Missions, Team, Rooms, Memory, Routines, Settings)
  and the sections inside Settings are now bold, widely spaced capitals,
  set the way the LOCUST wordmark is. Teammate replies keep their serif, and
  commands, paths and routes keep Geist Mono. Names are never capitalised:
  a folder named in a title keeps its own spelling.

- The decision cards' titles are drawn in the semibold they always asked
  for. Geist had no semibold, so they were showing at medium weight.

## 0.267.0 - 2026-09-22

- **The model picker fits more than twice as many models.** Every model took
  three lines, with its whole list of effort levels spelled out, so only
  three fitted on screen. Each model is now one line, the way Claude Code and
  Codex draw their own pickers: the name, what it is, and whether it is
  ready. Seven fit at once. Hover a model to see its full description and
  effort levels.

- **The app no longer stalls for a moment when a Codex reply finishes.**
  Closing Codex's background process held Locust for about a sixth of a
  second at the end of every turn, and again each time it checked Codex's
  model list. It now closes in the background.

- **Settings opens faster the second time.** Locust re-read your whole
  conversation history to show its size and age: about half a second on a
  138-conversation history. Now it re-reads only conversations that changed,
  which takes a few milliseconds.

## 0.266.0 - 2026-09-22

- **Claude Code command rows say what the command was for.** Claude writes a
  one-line description for every command it runs, like "Sleep for 8 seconds
  then write finished to out.txt". Locust has meant to lead with it since
  0.56, but for Claude Code it never arrived, and every row showed the raw
  command instead. Rows now lead with the description. Press the row to see
  the exact command underneath.

- **A command left in the background says what happened to it.** A
  teammate's run ends when it answers, and Claude Code stops anything it
  started in the background at that moment. The row used to say a green
  **done** beside "in the background", which read as finished work. It
  now says **stopped · when the run ended**, in amber. It says **done** or
  **failed** only when the work really finished, and **running** while it
  still is. Teammates are also told that nothing wakes them when background
  work finishes, so they wait for results they need before answering.

- **Haiku is in the model picker for Claude Code**, Claude's fastest and
  cheapest model. Claude Code accepts it, but its help text only names
  Fable, Opus and Sonnet, so Locust didn't offer it.

## 0.265.0 - 2026-09-22

- **The controls under the message box stay inside it.** At the usual window
  size the last button — the swarm mark — sat a few pixels past the box's
  edge, and further once a file was attached; at the smallest window it sat
  well outside. The row now fits at every size: a long folder name gives up
  a little more of itself, and on a narrow window the controls take two
  tidy rows instead of running off the end.

## 0.264.0 - 2026-09-22

- **Dialogs keep the keyboard.** In the routine editor, Tab walked out of the
  dialog to the window buttons and the screen behind it, and Escape did
  nothing once it had. The routine, teammate and group dialogs now keep Tab
  inside, close on Escape wherever focus is, and hand focus back to the
  button that opened them.

## 0.263.0 - 2026-09-22

- **Tables in a reply follow your text size.** With replies set to Large, the
  prose grew and any table stayed small. Tables now grow with it, a touch
  smaller than the prose around them.

- **Team cards line up.** Edit and Remove are small icons now, the way the
  Routines screen draws them, so a role like "Code & Migrations" stays on one
  line and every card in a row is the same height. A teammate that has never
  run no longer says its usage was "not reported by the runtime".

- The Details panel and the receipt say **Usage** for token counts and
  **Cost** only when there is a price, as the Team card already did.

- In the model picker, a group's note no longer runs into its heading.

## 0.262.0 - 2026-09-22

- **Long conversations stay smooth while a teammate works.** Whenever any
  teammate was streaming, Locust redrew the conversation on screen from
  scratch many times a second — every earlier turn, every fold, every diff.
  On a 30-turn conversation that took about 34 milliseconds each time, longer
  than a frame. Earlier turns are now drawn once and left alone until they
  change: about 3 milliseconds. Nothing on screen looks different.

- The Activity panel's timeline no longer slows the window down while it is
  open.

## 0.261.0 - 2026-09-22

- **Locust no longer stalls when a teammate finishes.** Every time a run
  ended, Locust read your whole conversation history back from disk to
  refresh the list — about half a second on a history of 138 conversations,
  and the app was slow to answer anything else while it did. It now keeps
  what it has read and only reads the conversations that changed: about
  forty milliseconds, and nothing waits on it.

- Reading your teammates and groups no longer slows down as the number of
  conversations grows.

## 0.260.0 - 2026-09-22

- **Locust opens faster.** On a machine with every coding agent installed it
  took 6.6–7 seconds to open; it now takes about 4.3. Most of the wait was
  checks that could not change the answer: Gemini CLI, which Locust lists but
  cannot run yet, was asked whether it was signed in on every launch and was
  the slowest check of all; Copilot and Muse were asked a question whose
  answer was already known; and finding Antigravity used a Windows command
  that takes most of a second where another takes a twentieth. The loading
  screen also lets go sooner once everything has answered.

- **Sending after a break starts right away.** The first message after a few
  quiet minutes used to wait while every coding agent was checked again —
  3 to 6 seconds before anything happened. Now only the one your teammate
  runs on is checked.

- **Coming back to the window is lighter.** With one coding agent signed out,
  every return to the window re-checked all of them. Now only the ones that
  are not ready are checked, and pressing **Sign in** makes the next return
  check at once.

- **Check again always checks.** Pressed within ten seconds of the last
  check, it could hand back that check's answer without asking anything.

## 0.259.0 - 2026-09-22

- **A teammate's face is enough.** Conversations and rooms no longer print
  the name beside the face: the face says who it is, and hovering it says
  the name. A reply that runs several lines shows the face once, and a
  room's answer card shows it once, in its header.

- **The first screen says whether to install or to sign in.** Each coding
  agent that is not ready reads **not installed**, with Install beside it,
  or **not signed in**, with Sign in beside it.

- **Numbered steps count 1, 2, 3.** A teammate that wrote its steps with a
  blank line between them got a list of its own for every step, each
  numbered 1. Steps now stay one list, a list interrupted by a code block
  carries on at the right number, and a second paragraph under a bullet
  stays inside it — which is also what broke the What changed page.

- **Ask mode stopping a run is one message.** It used to be a note with the
  way forward and then a red card saying the same thing again, with the
  runtime's raw words at the end. Now it is one note, the button, and the
  runtime's words folded under **What the runtime said**.

- **A plan says "not checked off", not "not reached".** A run that finished
  without ticking its checklist was drawn as if it never got to the steps,
  even when the work was done. The count now says what it knows.

- Settings: the Teammates section no longer promises automatic replies when
  they are switched off, and the note under the project folder lines up
  with the rest of its card.

## 0.258.0 - 2026-09-22

- **SIGN IN is a button now.** A runtime that needs signing in used to print
  a command and leave you to find a terminal. Press **Sign in** and Locust
  opens the runtime's own sign-in in a window of its own; finish there, and
  Locust checks again when you come back to it. It works for every runtime that
  needs an account, in Settings and on the first screen.

  A command-line tool cannot use the account your browser is signed in to, so
  the sign-in itself stays, once per runtime. For Muse Code it is a code you
  approve in the browser, which takes one click when you are already signed
  in to Meta there.

- **Muse Code is found signed in wherever it keeps its login.** On a machine
  that sets `XDG_CONFIG_HOME`, Muse keeps its login there rather than in your
  home folder, and Locust looked only in the home folder.

## 0.257.0 - 2026-09-22

- **A command sent to the background says so.** Claude Code can be told not
  to wait for a command, and Locust drew that call exactly like one it had
  waited for — which is most of why a background task that finishes reads as
  a turn that simply stopped. The row now says **in the background**.

  It says what the call was, not when the work ended. Nothing brings the
  teammate back yet.

## 0.256.0 - 2026-09-22

- **A turn no longer says `959 min in`.** Time markers count in units a
  person reads — `45 min in`, `5h 5m in`, `2d 4h in` — and once a
  conversation has carried on into another day, the marker names the day
  instead of leaving a bare clock time that could be from any of them.
- **The first screen lines up.** The logo card sat in the middle of the pane
  while everything under it started further left, so it looked off-centre
  against the things it was meant to head. One column now, and the opening
  line says Locust runs the coding agents you already have, on your own
  accounts.
- **Copilot's background tasks are recorded.** Locust has no background-task
  feature, and the one event any runtime sends about them was being dropped
  by name — so there was nothing to build one from. It is kept now.

## 0.255.0 - 2026-09-22

- **A plan stops promising work that is never coming.** When a run ends
  without its runtime closing off the plan, the steps it never got to used to
  sit under the finished answer looking exactly like steps still to come. The
  card now says how many were not reached, and those steps read as past. They
  are not quietly marked done — a plan that was not finished is worth seeing.
- **Gemini CLI stops offering modes it cannot run.** No mission can run under
  it at all, yet it listed four permission modes, which is a menu of
  refusals.

## 0.254.0 - 2026-09-22

- **The Rooms form says what each field is.** Room name and Who is in it are
  labels now, not placeholder text that vanishes the moment you type. The
  button sits beside a line saying how many teammates are in and that each
  one answers on its own route.
- **A picked teammate carries a tick**, not just a green outline — so which
  ones are in the room does not depend on telling two shades apart.
- **Rooms tells you where your first room will appear** instead of leaving
  the space under the form blank.
- **Saving a conversation as a routine is offered once you already have
  one.** The offer used to appear only on an empty Routines screen, which
  left anybody with a single routine and no memory of how they made it with
  nothing to go on.

## 0.253.0 - 2026-09-22

- **The Routines list is a card, and a routine is two lines instead of
  three.** Each one takes 54px now, against 105px two builds ago — four fit
  in the room one and a half used to take.
- **A routine's schedule is a chip you can scan**, not a third line of prose
  under the name. A routine with no schedule says "no schedule" rather than
  leaving the space blank, which read as something that had failed to load.
- **Edit and Remove are icon buttons**, so Run — the thing you came to press
  — is the only worded control on the row. Both icons carry a tooltip and a
  screen-reader name.

## 0.252.0 - 2026-09-22

- **New room is a card.** The name field, the teammate chips and the Create
  button used to sit loose on the screen with nothing holding them together,
  so the button read as unrelated to the field above it. They are one object
  now, in three parts: what a room is, what this one will be, and the button.
  The teammate list has a label with the count beside it.
- **A routine's owner sits beside its name**, not floating between the name
  and the line under it — and the detail line no longer opens with a stray
  "·" left over from when it ran on from the name.
- **The Routines list keeps its distance from the window edge.** Remove was
  flush against it, and on a narrow window it was cut off.

## 0.251.0 - 2026-09-22

Routines and Rooms, read properly for the first time.

- **The page stops jumping when you open Routines.** Routines drew a
  different header from every other screen — 129px against 60px — so
  everything below the title dropped 69px on the way in and rose again on the
  way out. Every list screen uses the same header now, and the count sits in
  it. Measured: a 69px step, then none.
- **A routine's name reads as the name.** The line beside it — the teammate,
  the step count, when it last ran — was set at the same size and colour as
  the name in a heavier typeface, so it won the line and the name lost it.
  The name is now the larger of the two and the detail sits quietly under it.
- **The owner's face sits beside the routine it belongs to**, instead of
  alone on a line above it. A routine takes about a third less room as a
  result: four now fit where three did.
- **Remove looks destructive when you reach for it.** Run, Edit and Remove
  were the same control in the same clothes; Remove turns red on hover.
- **Rooms has one title, not two.** "Rooms" and "New room" were the same size
  and weight, one above the other, so neither read as belonging to the other.
- **Create room says why it is unavailable.** It refused on three counts and
  explained only one of them: too many teammates named the number to untick,
  while no name and nobody picked were a greyed-out button and silence. All
  three say what is missing now, and the roster shows how many of them you
  have ticked.
- **Long paragraphs stop stretching to the window.** Settings and Rooms ledes
  had no measure, so they got harder to read the wider you made the window.

## 0.250.0 - 2026-09-22

- **A teammate on its own branch can read again.** In Ask or Plan mode, on
  the free OpenCode model, every such run came back with "OpenCode's free
  tier can only be used from within OpenCode" and nothing else. Three of the
  app's own defaults at once — the recommended free model, the mode a careful
  person picks first, and Own branch — and the combination was the only one
  that failed. The permission Locust sent for those runs denied the shell
  tool outright, and the provider reads a client with no shell tool as not
  being OpenCode.

## 0.249.0 - 2026-09-21

- **Muse Code says when nobody has signed in.** It used to read as ready on a
  machine that had never logged in, because the only way to ask Muse whether
  you are signed in is to start a run, and a run on Muse costs money. So the
  first thing you learned was a raw error in the middle of a mission. Locust
  now checks for the credentials file Muse itself names, which costs nothing,
  and the row reads **SIGN IN** with the command to run.

  An API key in `META_API_KEY` counts as signed in, the same as Muse treats
  it. And a machine Locust cannot look at is never reported as signed out —
  not being able to check is not the same as knowing.

## 0.248.0 - 2026-09-21

- **A Muse Code mission actually starts.** Every one of them stopped at
  "the mission ledger could not be written" — before Muse was launched, so
  nothing ran and nothing was changed, but nothing worked either. The ledger
  keeps its own list of runtimes it will record, and Muse Code had been
  added everywhere else and not there.
- **Routines can run on Muse Code too.** They were held back by a second
  list with the same gap.

## 0.247.0 - 2026-09-21

- **Muse Code runs missions now.** Meta’s CLI has been in the runtime list
  for a day as a roadmap row that did nothing. Locust can read its output, so
  it is selectable, it shows up in the model picker, and a mission under it
  is recorded like any other — same thread, same receipts, same history.

  It is marked PREVIEW on purpose. Every part of the path has been run end to
  end, but only against Muse’s own free test provider, which never calls a
  model and never uses a tool. Nobody has watched it do real work yet.
- **Ask, Accept edits and Plan on Muse Code, and not Auto.** Auto means a run
  may edit anything on this machine, and Locust keeps Muse’s own sandbox on
  whichever mode you choose. Offering Auto would have promised more than the
  run actually gets.
- **The mode menu names the runtime you actually picked.** With Muse Code
  chosen, the two modes it cannot offer explained themselves as "Codex CLI
  only" and "Codex CLI runs its own agent" — about a runtime that was not in
  play.
- **The Install link on Muse Code works.** It was the only runtime whose sole
  way in is a vendor page, and pressing the link did nothing at all.

## 0.246.0 - 2026-09-21

- **A long conversation stops turning into several.** Past 32 turns, the
  sidebar started drawing one conversation as two rows, then three, then four
  — a new one every turn after that. Colin saw it as Antigravity "spawning a
  new conversation after a workflow finishes"; nothing was spawning, and his
  36-turn chain was intact in the record the whole time. The list now follows
  a conversation back however long it is.

  If you tidied one of those phantom rows into a group, that conversation may
  still be filed in two groups. Moving it once more puts it right.
- **The model picker stops telling you OpenCode needs an account.** Every
  group in it was labelled "· your account", including the one runtime Locust
  recommends because it needs none.
- **Runtimes read in the same order in the picker as in Settings.** They were
  sorted by whichever answered its version check first.

## 0.245.0 - 2026-09-21

Settings, rebuilt to look like the rest of the app.

- **Settings rows live in a card now.** They used to float on the window
  background with six pixels of padding and a hairline running edge to edge —
  the single thing that made the screen read as unfinished next to a
  conversation. Same card, same border, same corners as every other row.
- **Rows have real height and real padding**, and light up under the pointer,
  matching the sidebar rows three inches to their left.
- **The 2 / 4 / 8 / 12 / 16 / 24 picker is one control**, not six buttons at
  full size. Its digits line up.
- **The Settings menu marks the page you are on the way the sidebar does.**
  Two navigations in one window had been using two different languages.
- Sections sit closer together, so two groups read as one page.

## 0.244.0 - 2026-09-21

The first hour, from a beta review driven on a blank machine.

- **The first message cannot spend.** Install OpenCode — the one agent Locust
  recommends because it needs no account — and the composer now names a free
  model instead of "Account Default". Account Default was whatever the account
  gives you, which on a signed-in account is a paid model somebody else chose,
  sitting behind the first Enter.
- **The model picker opens on what actually works.** It listed three agents
  that were not installed, and one that does not exist yet, above the only one
  that could run. Ready first now, then signed-out, then not installed.
- **Send says why it is grey.** It used to describe the thing it would not do.
  On a machine with nothing installed it now says so, ahead of every other
  reason, because that is the one typing cannot fix.
- **Stopping a run no longer argues with itself.** "You stopped this run" sat
  directly above "Nothing had started yet". It says "Stopped before it did any
  work" — about the run, not about you.
- **Ask mode offers the mode that works.** Asking Ask mode to write a file
  correctly refused, then offered "Run it again" beside a sentence admitting
  the rerun could not change anything. It offers the switch to Accept edits.
- **"What changed" stays on Home.** The version banner sat over live
  conversations until dismissed.
- **Two sentences stopped being wrong about the list under them.** The
  collapsed "others Locust can drive" line claimed they each need an account,
  which was untrue of two of five; and "Node.js is not on this machine" stayed
  on screen under an install that had already finished.

## 0.243.0 - 2026-09-21

- **Switches line up with their own tracks.** Every toggle in Settings pushed
  its knob two pixels too far when switched on, so it sat against the edge
  rather than inside it; the switches on the Memory screen stopped two pixels
  short of the end, reading as not quite on. Both now travel exactly the room
  they have.

## 0.242.0 - 2026-09-21

- **A teammate can name a memory, and naming it stops it piling up.** Facts
  that change — a test result, a version, whose turn it is — were written
  fresh every time, so the same fact accumulated a dozen near-identical
  copies and crowded out everything else. A teammate can now file one under a
  name, and remembering that name again rewrites it instead of adding another
  copy. What it said before is kept, and the thread says "updated" rather
  than "remembered" so a memory never changes under you silently. Memories
  without a name behave exactly as they did.

## 0.241.0 - 2026-09-21

- **The welcome screen stops promising a free teammate if there is not one.**
  It told every new person "OpenCode needs no account — one install and you
  have a working teammate". Whether OpenCode still publishes a free model is
  somebody else's decision, and nothing checked it. Locust now checks: if
  OpenCode is installed and none of its models are free, the sentence says so
  instead. If we have not looked yet — which is normal before anything is
  installed — the promise stands, because not having looked is not the same as
  finding out it is false.

## 0.240.0 - 2026-09-21

- **The send button's cursor bend is off unless you ask for it.** The liquid
  dent that follows your pointer across the ring was on for everyone; it is a
  flourish, so it now starts off and lives in Settings for anyone who wants
  it. If you had already turned it on, it stays on.

## 0.239.0 - 2026-09-21

- **A routine saved from a conversation runs the way that conversation ran.**
  Saving a routine from a conversation nobody owned stored the permission
  mode as "Ask", whatever the conversation had actually used — and on a
  route that cannot be held read-only, that routine could never start at all.
  It now takes the mode the conversation really ran in, and follows whichever
  teammate you pick when it asks who should run it.
- **A routine that was refused says why it was refused.** When a runtime turns
  a routine down — "Cursor Agent cannot be held read-only on this system",
  say — that sentence is what the card shows. It used to be replaced,
  moments later, with "the app stopped before saving a mission receipt":
  blaming a crash that never happened and sending you to review work that had
  never started.
- **Antigravity says when it is probably waiting for you.** Antigravity asks
  its own questions in its own window, where Locust cannot see them, so a run
  could sit silent for a long time looking like it was working. After a
  minute and a half of silence with a step still open, the thread now says so
  and names the step. The run is not ended — it is a note, not a verdict.

## 0.238.0 - 2026-09-21

- **A long answer no longer stops the run that wrote it.** A Cursor run that
  had already printed a full account summary on screen ended with "the
  mission ledger could not be written — too many mission events in one
  append", and the work was thrown away. The limit it hit is about how much
  Locust writes to disk at once, not about how much a run may say; a burst
  bigger than that is now written in several goes instead of being refused.
  The ledger from that run was intact all along — 547 records, nothing
  wrong with it.
- **Locust asks a CLI its version and its help text only when the CLI has
  changed.** Both answers are fixed by the program on disk, so they are
  remembered between launches and re-read when the file is updated. Whether
  you are signed in is still checked every time, because that changes
  without the file changing. Measured on the machine this was built on: a
  second launch finds every runtime in 4.2 seconds where the first takes
  6.1, with the same answers.

## 0.237.0 - 2026-09-21

- **A teammate is handed the memories that bear on what you asked.** The
  brief pasted the newest eight memories and left the rest in a file the
  teammate could open. The eight were chosen by date alone, so a note from
  weeks ago that answered your question exactly was left out in favour of
  whatever was written most recently. The memories that share words with your
  message come first now, then the newest as before; the file is unchanged.
  Measured on the free model before the change: it did open the file and
  find the answer — this saves it the trip.

## 0.236.0 - 2026-09-21

- **Locust stops re-checking runtimes that have nothing to report.** Every
  return to the window, and a timer fifteen seconds after launch, re-ran the
  whole runtime sweep — six seconds on the machine this was built on — even
  when every installed runtime was already ready. Those sweeps now run only
  when something installed is not ready (signed out, or not yet answered).
  Check again still checks everything.
- **The window appears sooner.** Locust asked npm where it keeps its
  programs before it drew anything; that question is asked in the background
  now and read only if a runtime is not found on PATH.
- **Antigravity is found without two PowerShell round trips per sweep.**
  A cheap process check decides whether Antigravity is open and whether it
  is the same one as last time; PowerShell runs only when that has changed.

## 0.235.0 - 2026-09-21

- **Locust finds your runtimes in half the time.** Each runtime's checks —
  version, help, sign-in, model list — ran one after another, and eight
  runtimes' worth of that took twelve seconds on the machine this was built
  on. They run together now, and a check that answers two questions is asked
  once: six seconds, same answers. A single slow check is also given ten
  seconds rather than five before it is called a failure, because a check
  that is merely slow was being reported as broken and re-asked.
- **A teammate's replies row wears its name from the start.** 0.234 named
  the row on disk but the sidebar kept showing the first message's words
  until the next launch.

## 0.234.0 - 2026-09-21

- **Each teammate has one conversation their replies go to.** When a
  teammate wrote to another, the reply started a new conversation every
  time, and the sidebar filled with them. A reply that is not part of an
  exchange you can see now continues that teammate's own conversation —
  named "Wren's replies" the first time it is made, renameable like any
  other — and clicking their face at the top of the sidebar opens it. The
  "only their conversations" filter the face used to be moved into the card
  that opens when you hover a face, which is where their conversations are
  listed anyway.

## 0.233.0 - 2026-09-21

- **A gap the runtime left is marked as one.** Antigravity notes its own
  dropped bytes inside the text it sends, so the note arrived mid-sentence in
  a teammate's reply, in their voice, splitting a word across it. The words
  either side are untouched — nothing is hidden, because something really was
  lost — but the note is now drawn as Locust saying it, not the teammate.

## 0.232.0 - 2026-09-21

- **The mark on a running conversation is clean again.** Shrinking it a
  quarter in 0.226 put its dots under a screen pixel: it is the one drawing in
  the set that keeps three quarters of its points at small size, where the
  others keep a tenth to a third and draw them fatter. It is back at its own
  size, where it reads as a circle rather than a smudge.

## 0.231.0 - 2026-09-21

- **The window opens narrower.** It opened up to 1280 wide, which on a big
  display left a column of empty panel beside the thread — the app looking
  like it had nothing to put there. It opens at the width the app is actually
  worked at now, and still takes a fraction of smaller screens rather than a
  fixed number.

## 0.230.0 - 2026-09-21

- **Sending starts the run instead of re-checking every runtime first.**
  Starting one mission re-probed all of them — version, help, readiness and
  model list — to answer one question about the one it was using, which on a
  slow machine was several seconds between pressing Send and anything
  happening. The Stop button appeared immediately, so the screen said running
  for the whole wait. A runtime that was ready in the last few minutes is now
  taken at its word; anything else is still checked properly, because that is
  the case where the answer may have changed.

## 0.229.0 - 2026-09-21

- **"Downloaded" names the version that is actually downloaded.** If a newer
  release appeared after one had been fetched, the window kept saying an
  update was ready and put the NEWER number on it — so Restart and install
  came back on the same version with the same button showing. It only happens
  when releases land close together, which is why it took until tonight to
  surface. A fetched update is now offered as ready only while it is the one
  the check just found; anything newer goes back to offering Download.

## 0.228.0 - 2026-09-21

- **A run that is writing says so, on every runtime.** Text arriving is the
  model writing, and the live line now shows that — which on runtimes that
  report their tools only once those tools have finished was the difference
  between one mark for a whole run and a line that actually changes.

## 0.227.0 - 2026-09-20

- **A refusal reads as a refusal, not a crash.** In Ask mode, a run that tried
  to use a tool the mode does not allow ended with "OpenCode ended without a
  step that reported it had stopped" — so the safety boundary working
  correctly looked like a broken runtime. It now says which tool the mode
  refused, and that nothing was changed.
- **Free routes stop being described as priced.** A run whose receipt reports
  tokens and no price is a measurement, not a charge: Missions says "measured"
  rather than "priced", and the Team card labels those numbers Usage rather
  than Cost.
- **"Save conversation as routine"** says what it does. It builds a routine
  from the whole conversation, not from the one mission you opened it on.

## 0.226.0 - 2026-09-20

- **A run that is cut off mid-sentence no longer leaves plumbing on screen.**
  When a turn is interrupted inside one of the blocks Locust reads, the stream
  stops mid-word — so what reached you was the first few characters of a tag
  with nothing to close it, sitting at the end of the reply as though the
  teammate had typed it. It comes off now. A stray angle bracket in ordinary
  prose is left exactly where it is.
- **The mark on a running conversation is a quarter smaller**, which is safe
  on an outline in a way it would not be on any of the others.

## 0.225.0 - 2026-09-20

- **The step underway is one solid mark.** The orbiting particles read as
  diffuse next to a single named piece of work; it is the small dense sphere
  now, at the size that drawing was made for.
- **A running conversation in the sidebar wears the dotted outline**, and at
  full size rather than shrunk. The orbiting particles were shrunk because
  they were the loudest thing in a row whose job is to name a teammate — but
  a cloud of points has no silhouette to keep, so it read as noise at any size
  that row can spare. An outline stays itself all the way down.

## 0.224.0 - 2026-09-20

- **The plan card says it is running, where you can see it.** The scrambling
  sphere moved off the individual step and up beside PLAN at a size its bands
  can actually be read at. It appears only while a step is underway.
- **The step underway gets its small orbiting mark back**, which is what a
  row marker is the right size for — and the rows line up again.
- **The plan reads in mono**, like the card's own header and ordinals. It was
  the only part of that card speaking in the prose face.

## 0.223.0 - 2026-09-20

- **The live line reads like a line again.** The word beside the orb was
  10.5px in the monospace face, which next to a bigger orb read as a caption
  on a picture rather than a sentence. It is 14px in the app's own UI face
  now, in the app's own medium weight, capitalised, with an ellipsis after it
  — three dots, which is what people write, rather than the four the
  library's own page happens to use.
- **The travelling highlight is brighter, and safer.** It is the library's own
  technique now: the word stays solidly painted and a bright band slides over
  a copy of it, instead of the word itself being transparent ink over a moving
  gradient. It can be bright without changing the resting colour, and with
  motion turned off the highlight simply goes away.

## 0.222.0 - 2026-09-20

- **The orbs look like the orbs.** The library ships two separate drawings per
  orb — one tuned for 64px and one for inline text at 20px — and Locust was
  showing the small one everywhere. At 20 the connector orb keeps a fifth of
  its points at one and a half times the size, which is why it looked like
  nothing on the library's own page. The six whose shape lives in their
  detail now use the big drawing, painted down into a slightly larger box, so
  it stays sharp.
- **The two outline orbs keep the small drawing**, because that is the one
  that reads: at the big size their lines are hairline and fade into the
  panel.

## 0.221.0 - 2026-09-20

- **Conversations stop disappearing.** The sidebar only ever received the 20
  most recently touched conversations, so one you had not opened in a while
  was not hidden or renamed — it was absent, and its group read empty. It now
  lists every conversation you have. Twenty was a budget on how much
  TRANSCRIPT one message could carry, which a row does not need: the newest
  still arrive with their transcripts, the rest arrive as rows and fetch
  theirs when you open them.

## 0.220.0 - 2026-09-20

- **The orbs are back to one size.** 0.218 drew one of them larger, on the
  wrong row — and larger turned out to be the wrong idea anyway: these are
  drawings made for their size, so scaling one softens every stroke, which on
  a mark this small reads as broken rather than as big.
- **The plan's step has its rubik sphere back.** It lost it when that shape
  went to every open tool, because the plan and the live line share a screen
  and two identical spheres a few pixels apart read as a glitch. The shape
  belongs to connectors now, which are rare and brief, so the plan can have
  it again.

## 0.219.0 - 2026-09-20

- **A connector call is recognised as one, whatever the runtime calls it.**
  Every MCP name Locust had ever seen carried its server, so a runtime that
  names the tool flatly `mcp` had its connector calls filed as ordinary local
  tools: no connector row, and nothing saying the work had left your machine.
  They are connector calls now. When the name carries no server, the line says
  "using a connector" without naming one, because it does not know.
- **MCP and connectors have their own animation.** The scrambling sphere is
  theirs; ordinary tools and shell commands take the one it used to share with
  them. The rarer event gets the louder mark — a tool call already has its
  name on the row beside it, and a call leaving your machine is the one worth
  spotting without reading.

## 0.218.0 - 2026-09-20

- **The sweep on the live line is wide enough to see.** It shipped in 0.217
  and read as nothing: the bright band covered a fifth of its travel, so on a
  seven-letter word it was a couple of characters passing in a blink. It is
  twice as wide now and peaks a step brighter — still only lightness moving,
  never a colour of its own. What it is for: the word beside the orb is live,
  not text that got stuck there.
- **The thinking orb is drawn a size up.** It is a wave through latitude
  rings rather than a solid ball, so it used less of its box than the denser
  shapes on the other rows and read smaller at the same size. The line does
  not move when it changes.

## 0.217.0 - 2026-09-20

- **A finished run no longer says things are still running.** A turn that had
  ended could still show rows reading "running" and "still running" — while
  the summary line above them already counted them correctly as "did not
  report". The rows say that now too, in amber: a tool that never reported
  back is not a tool that failed.
- **The live line has a sweep.** A band of lighter ink travels along the word
  for whatever is happening, and along the name of a tool while it is open —
  the thing Claude Code does to its active line.
- **All nine orbs are in use, and the ones you see most are the fullest.**
  Working, thinking and open tools carry the three roundest; reading and web
  search share the globe; a connector and a sub-agent are now told apart;
  writing and the plan's step take the rest.

## 0.216.0 - 2026-09-20

- **Older conversations stop dropping out of their groups.** A conversation
  that began far enough back could appear under a different name and outside
  the group you filed it in — because the sidebar only ever saw the twenty
  most recent turns, and a conversation whose first turn fell outside that
  window was identified by a later turn instead. Its name, and which group it
  is in, both hang off that first turn. Nothing was ever moved, renamed or
  lost; the app was looking under the wrong name.
- **The orbs are spherical where you look most.** The plan's step underway
  has its own orb rather than borrowing the live line's, and the busiest rows
  carry the fuller shapes.

## 0.215.0 - 2026-09-20

- **The send button's halo leaves with your cursor.** It used to stay lit
  after you moved away — still, but still glowing — because stopping the
  effect froze it on the frame it happened to be on, and that frame was the
  bright one. At rest the button is a plain metal ring again; the glow is the
  part that answers you.

## 0.214.0 - 2026-09-20

- **A teammate's message never shows you the plumbing.** Locust reads a few
  tagged blocks out of what a teammate writes — send this to Wren, remember
  this, ask Colin. When one was written slightly wrong, it was both ignored
  AND printed: the reply arrived with `<locust-share>` around it. Now the
  tags come off whatever is still wearing them and the words stay, so a
  message that went nowhere is at least something you can read and pass on
  yourself.

## 0.213.0 - 2026-09-20

- **The cursor actually bends the send button's ring now.** The setting was
  there and did nothing: the numbers were written where the library keeps
  them, but the part that reads them every frame was never switched on. Move
  the pointer across the ring and it dents toward you and springs back.
- **The glow lets go.** Press send with the cursor still on the button and
  the button disables under your pointer — which means it never hears the
  pointer leave, so the metal kept moving over an empty composer. It stops
  now, and it stops for the other ways a cursor can leave without saying so:
  a window losing focus, a pointer leaving the window.
- **Silver at standard is the default metal.** Every option is still in
  Settings; this is just where it starts.
- **The orb on a working conversation is smaller**, so the row still reads as
  a teammate's name first.

## 0.212.0 - 2026-09-20

- **Settings now has every option for the send button's metal.** Which metal
  (off, chromatic, silver, gold), how strong, whether it moves only on hover
  or all the time, and whether the cursor bends the ring as it crosses. Off
  means no shader at all rather than a paused one.
- **The orb sits beside the word it is about.** It was before the teammate's
  name, which read as a property of the teammate — it is a property of what
  they are doing.
- **A running conversation in the sidebar shows an orb instead of a dot**,
  and goes back to a quiet dot when it finishes.
- **The plan's step underway carries the same orb** the line below it does,
  instead of its own pulsing pip.
- **Teammates' faces move again while they work.** They were stilled when the
  orb arrived, on the grounds that two moving things say one thing twice —
  but the face says *this teammate is alive* and the orb says *what kind of
  work*, which are different claims.

## 0.211.0 - 2026-09-20

- **A picture a teammate made opens as a picture.** Charts, diagrams and
  screenshots used to be refused by the file panel with "Locust does not open
  that kind of file here" — about a file your teammate had just made for you.
  PNG, JPEG, GIF, WebP, BMP, AVIF and ICO now draw in the panel. SVG
  deliberately still opens as code: an SVG is a document that can carry
  script, and Locust does not run what a teammate wrote.
- **The send button turns to metal while you are on it.** Only that button,
  only on hover or keyboard focus, and it goes quiet the moment you leave —
  in this app a thing that moves means work is happening, and an invitation
  is not work.

## 0.210.0 - 2026-09-20

- **The orb changes at every word the line changes to.** Starting, thinking,
  working, writing and a connector call each have their own now, instead of
  four of them sharing two. A run you watch actually moves through them.
- **Fixed: the orb said "thinking" while the line said "using a tool".** On
  Codex and Claude Code, which report a step as well as its tools, a step that
  named a tool showed the waiting orb beside it. Seen only by running a real
  turn on each.

## 0.209.0 - 2026-09-20

- **The orb is there for the whole wait now, and it changes when the work
  does.** It used to appear only while a tool was actually open, which meant
  it never appeared at all on runtimes that report their tools once they
  finish — and on every runtime, the long wait before the first tool showed
  nothing. A run that has not answered yet shows a slow ring; once it is
  working the ring becomes moving particles; reading, running a command,
  waiting on a subagent and planning each keep their own. The three dots
  retire wherever an orb appears, so "still going" is said once.

## 0.208.0 - 2026-09-20

- **The running step shows what kind of work is happening, as a small moving
  orb.** Reading a file, running a command and waiting on a subagent each get
  their own; a Plan-mode turn gets the planning one. It is tied to the work
  rather than cycling through shapes, so it can never say "reading" while the
  line beside it says a command is running. Where none of them is true — while
  a file is being written, or while the teammate is just waiting on the model —
  there is no orb and the line keeps its three dots. The teammate's face sits
  still while an orb is moving, so the row says "still going" once instead of
  twice.

## 0.207.0 - 2026-09-20

- **All six coding agents fit on the first screen now, in two columns.** With
  six installed, the list drew five and left a scrollbar whose whole job was
  to reveal the sixth. A machine with nothing installed still sees the one
  row it saw before.
- **A file in the panel is sized to be read.** The panel's width was a
  percentage taken off one screenshot, which meant a different line length at
  every window size. It is now set by counting the characters the column
  actually renders, and stops growing once a line is long enough to read
  comfortably — past that the panel would only be taking room from the
  conversation.
- **The panel says why it will not open a file.** "Locust does not open files
  — a teammate chose this file's name and contents. Reveal hands it to
  Windows." A missing button reads as an oversight; this is a decision, and
  now it says so.

## 0.206.0 - 2026-09-20

- **An open file says which turns changed it, and you can look at each one.**
  A strip under the file's name counts the turns of this conversation that
  touched it — "changed in 2 turns" — with a numbered button for each and
  "Now" for the file as it stands. Pressing a number shows what that turn
  changed, with the ask that caused it on the button's tooltip. It says
  *changed in*, never *as it looked*: the record is a list of changes, some of
  them cut short by the runtime that reported them, so Locust will show you
  the changes rather than rebuild a document it cannot vouch for.

## 0.205.0 - 2026-09-20

- **The conversation keeps its room when a file is open on a smaller window.**
  In a 1120-wide window the open file panel was squeezing the conversation
  into a column half the panel's width — replies breaking after three words,
  "Write a message..." on two lines, the model's name cut off. The panel now
  floats over the workroom the way the activity drawer already did, so the
  conversation beside it keeps three times the width it had.

## 0.204.0 - 2026-09-20

- **Any file a teammate touched opens beside the conversation, not just one
  they handed you.** The activity fold lists every file a turn wrote or
  changed, and each row now has a button that opens it in the same panel. The
  diff above it tells you what changed; this tells you what the file says,
  which for a report or a brief is the whole question. As everywhere else in
  Locust, it renders the file itself and never asks Windows to open it.

## 0.203.0 - 2026-09-20

- **Click a file a teammate handed you and it opens beside the conversation.**
  Markdown is rendered the way a reply is; anything else is shown as code. The
  panel has its own buttons to show the file in your file manager or save a
  copy. It never hands the file to Windows to open — Locust draws it.

- **The plan card has its border back, and its finished steps are filled in.**
  It was drawn without either, which was never what the design called for.

## 0.202.0 - 2026-09-20

- **Check again re-asks everything, including npm.** Whether npm is on your
  machine was decided once per session, so if yours was slow or hung the
  first screen said so and nothing could change its mind until you pressed
  Install. The button that exists to ask again now asks again properly.

- **A failed check no longer stops Locust checking.** If the app could not
  read the machine once, it gave up asking for the rest of the session and
  the screen quietly kept whatever it had.

## 0.201.0 - 2026-09-20

- **A file a teammate hands you has a save button.** The little download icon
  other clients have, for when you want the file somewhere else. It opens the
  ordinary Save dialog and copies it wherever you pick.

- **The note on a handed file is readable when it does not fit.** It shortens
  to fit the row, and the whole of it is in the tooltip now rather than lost.

- **And the card says so when the file is not there.** A teammate can name a
  file it never actually wrote. Pressing it used to do nothing at all, which
  looked exactly like it having worked.

- **The message box counts.** With five coding agents installed and none of
  them answering, it said "A coding agent is installed but not answering" —
  singular, over five rows that each said so.

- **"On this machine" means on this machine.** The count above the agent list
  was counting every agent Locust can drive, including ones you have not
  installed.

## 0.200.0 - 2026-09-20

- **A coding agent that is on your machine is always shown.** Yesterday's
  first screen tidied the list down to one step, and it tidied away installed
  agents too — so a Codex that was installed and not answering hid behind
  "they each need their own account", which was untrue of it, along with the
  Check again that would have fixed it.

- **"npm did not answer" no longer reads as "Node.js is not installed".** On a
  machine where Node is present and npm hangs, the screen said Node was
  missing. It now says what actually happened. The install works either way.

- **Nothing is left running when you quit during a check.** A coding agent
  probe started in the last moment before Locust closed could outlive it.

- **The install line says what it is doing** while it works out which npm to
  use, instead of claiming npm is starting.

## 0.199.0 - 2026-09-19

- **The reply is back to its old size, and the size is now yours.** 0.198.0
  set it larger on my reading of a measurement, and on a real screen it was
  far too big. It is back to what it was, and **Settings → Appearance → Reply
  text size** offers Standard, Large and Largest if you want it bigger.

- **The line length keeps the fix.** The thing actually worth correcting in
  0.198.0 was that a reply could run 111 characters to a line, well past the
  point where your eye loses its place coming back. Lines are about 74
  characters now, at whichever size you pick.

## 0.198.0 - 2026-09-19

- **A teammate's reply is set bigger, and its lines are shorter.** The reply
  was the smallest thing on screen that you actually read rather than scan,
  and its lines ran to 111 characters — past the point where the eye loses
  its place coming back to the left. It is now 18px with a shorter line.

- **Emphasis in a reply is real.** The reply's typeface ships no italic, so
  every emphasised word was a slant the browser invented by shearing the
  upright letters. Emphasis is now carried by weight, which the typeface
  really has.

- **Locust tells you when it made a folder for you.** Opening Locust from the
  Start menu gives it nowhere to work, so it makes a folder in Documents
  and works there. It used to say so only if you hovered over the folder
  button. It says so on the first screen now, with the button to point it at
  your own project.

- **The first screen shows one step instead of six.** With nothing installed
  yet, it offers the one coding agent that needs no account, and the other
  five are one line away when you want them. The explanation about Node.js
  waits until you have actually started an install.

## 0.197.0 - 2026-09-19

- **A run that has not answered yet says Starting, not Working.** The moment
  Locust had launched a coding agent, the line under your message said the
  teammate was working — on the strength of a process having been spawned,
  before the agent had said a word. One measured run showed "working" for over
  two minutes of complete silence. It says Starting until the agent actually
  reports something, which is also when the clock stops being a guess.

## 0.196.0 - 2026-09-19

- **A conversation you started without picking anyone can be saved as a
  routine.** The menu item was there and greyed out, and the reason it gave —
  "nothing here was typed by you" — was not true: you had typed every word of
  it. What was actually missing is whose turn it replays on, so the dialog now
  asks that once and saves it like any other.

- **The model picker reads as names.** A model whose program reports no name
  for it was listed by its identifier, so one row read
  `muse-spark-1.3-contributor-free` in a list where everything else read as a
  proper name — and the chip under the message box, an inch away, spelled the
  same model out properly. They match now. A name a program does give is
  printed exactly as it wrote it.

## 0.195.0 - 2026-09-19

- **A teammate can hand you a file.** Ask for one — "send me an md of your
  report" — and the file arrives in the conversation as a button under the
  reply, with its name and a line saying what it is. Pressing it shows the
  file in your file manager. Until now you could send files to a teammate
  but they could only tell you where they had put theirs.

- **A teammate stops messaging the others just to keep them posted.** Telling
  a teammate something starts a whole run on their side, and the rule they
  were given — pass it on if they need to know it — read as an invitation to
  share anything interesting. They now message a teammate when you asked them
  to, or when the turn made work that is genuinely theirs to do.

## 0.194.0 - 2026-09-19

- **A bullet that wraps stays one bullet.** When a list item's sentence ran
  past one line, everything after the first line was drawn as a separate
  paragraph below the bullet, at the left margin. It showed up wherever a
  list is read: in what a teammate writes back, and in this changelog, which
  is how it was spotted.

## 0.193.0 - 2026-09-19

- **The decision card says one thing once.** When a teammate stops and asks
  you to choose, the card told you twice that you could answer in your own
  words instead — once between the question and the buttons, and again under
  them. It says it once now, under the options, where it reads: here are the
  choices, and you are not bound by them.

## 0.192.0 - 2026-09-19

- **A to-do list looks like a to-do list again.** When a teammate kept a list
  while working and then answered without changing any files, the list was
  drawn as though the plan itself were the answer: a bare numbered line, in
  reading size, with no heading, sitting in the middle of the conversation
  where it read as a glitch. That treatment belongs to Plan mode, where the
  plan *is* what you asked for. Everywhere else the list is headed *PLAN ·
  n of m done* with a mark against each step, which is what it was before.

## 0.191.0 - 2026-09-19

- **A message sent without picking a teammate knows the project.** The home
  screen invites one: write below and assign it to someone later. That run
  used to be told nothing at all, because everything Locust briefs a run
  with hung on there being a teammate. It did not know which folder it was
  standing in, it never read the folder's `LOCUST.md`, and it could not
  answer a question about something you had typed on the Memory screen that
  morning: *"I don't have a secret word for this project in my
  instructions."* It now gets the folder and the project's memory, which
  belong to the project rather than to any teammate. What it still does not
  get is what needs a teammate to exist: a role, the roster, and the ability
  to hand work to someone else.

## 0.190.0 - 2026-09-19

The other half of the three tester reports, from Fable's reading of the code.

- **Nothing left running when you quit mid-check.** Closing Locust while it
  was still asking a hung CLI for its version left that CLI and its child
  behind (ten processes on Linux). Every check still out is ended when the
  app leaves.
- **A hung npm no longer holds the first screen.** One `npm` on PATH that
  never answers kept the screen at *checking the runtimes on this machine*
  for ever, with no rows and no Install. npm gets five seconds; if it does
  not answer, Locust uses the copy it carries.
- **Check again means four checks.** The rows said NOT ANSWERING after two
  real sweeps while the tooltip promised four, and kept checking every
  fifteen seconds after giving up. Sweeps are counted now, Check again
  starts the count over, and nothing is asked again after the fourth until
  you press it.
- **The "what changed" banner keeps its promise.** It is held until a
  runtime connects. It used to mark the version as seen the moment the
  changelog was read, so if nothing was connected at launch you never saw
  it, on that launch or the next. It is marked seen when it is on screen.
- **Installing with a non-ASCII Windows user name.** The small `node` stand-in
  the app writes for an install is read by Windows in the console's own
  code page; a name like José or 张伟 in the path broke it. It switches the
  console to UTF-8 first.

## 0.189.0 - 2026-09-19

Three tester reports landed at once (Grok's passes 13 and 14, Fable's
first). This build is the small half of what they found; the rest is in
`docs/PLAN-2026-09-19-TESTERS.md`.

- **A room's file agrees with its screen.** After everyone in a room had
  answered, the room file still listed the last teammate as waiting. The
  screen was right, the file was wrong, and anything reading the file would
  have started them again. Fixed at the write, with a test on the bytes.
- **No red card after hung CLIs give up.** Five rows already said NOT
  ANSWERING with Check again beside each; a card above the message box said
  to install one. The card now appears only when a runtime wants a sign-in,
  and the message box under not-answering rows says *installed but not
  answering — Check again above* instead of *install*.
- **Save as routine on a follow-up.** A conversation started with nobody
  picked and assigned afterwards could not be saved as a routine from a
  later turn: the menu said nothing had been typed. The owner is found along
  the conversation now.
- **"Plan mode — nothing was changed" only in Plan mode.** It was drawn under
  any reply that answered a question without touching a file, including in
  Accept edits.
- **The rail's flyout closes when you leave it.** Pinning a teammate on the
  narrow sidebar and then opening a room left their card floating over the
  room's answers.
- **The first words no longer blink out.** On a cold start the bold
  *OpenCode needs no account* could be blank for a moment while its weight
  loaded; the fallback face shows instead.
- **Two sentences made true.** The install line under the list now shows the
  command that actually runs with the app's own npm, and a memory file that
  fails to write is now logged, as 0.184.0 said it would be.

## 0.188.0 - 2026-09-19

- **The inspector stays with its conversation.** It used to stay open across
  All missions, Settings and Team, where it inspected nothing on screen and
  squeezed those screens until, at 1120 wide, the mission rows and the
  Settings pane grew sideways scrollbars. It now shows only beside the
  conversation, and comes back when you return to it.
- **Mission rows in a narrow list.** Below about 820px of list the row takes
  two lines instead of cutting off its last three columns.

## 0.187.0 - 2026-09-19

- **The smallest window, with everything open.** At 1120×720 with a long
  conversation, eight teammates and the inspector open, three things were
  cut off and are not now. With the sidebar folded to its rail, the inspector
  slid over the conversation and hid the end of every line, the receipt, the
  banner's Dismiss button and the header's own Activity button; the
  conversation now keeps left of it. The row under the message box (mode,
  folder, route, effort) was cut at its right end whenever the box was
  narrower than about 720px; it takes a second line there instead. And on the
  first screen, when six coding agents and the banner made it taller than the
  window, the Locust lockup was cut off at the top with no way to scroll to
  it; the screen now opens at its end, where Install is, and scrolls up to
  the lockup.

## 0.186.0 - 2026-09-19

- **The first screen, redrawn.** Everything on it now shares one left edge
  with the message box, so it reads as a sequence: which one do I pick, pick
  it, what happens when I click, and then what. The runtime list is one
  column instead of a grid with a hole in its corner, and each row says what
  it costs: *no account needed*, *needs a ChatGPT account*, *needs a Cursor
  account*. While Locust is still checking, the dots pulse and one line says
  *checking 6 on this machine* instead of six rows saying CHECKING. The
  recommendation is the one filled button.

## 0.185.0 - 2026-09-19

- **The first screen says less, and in order.** With nothing installed it
  no longer shows a red card saying no runtime can run; the disabled message
  box already shows that. The "coming soon" line is gone from it too, since
  it named things you cannot install on a screen for installing; Settings
  still lists them. And "here is what changed" waits until a runtime is
  connected, because on a fresh profile there is no previous version to
  compare with.
- **While Locust is still checking a CLI, it says only that.** The red card
  and the note about installing without Node.js no longer appear over rows
  that are still checking.
- **A CLI that never answers offers "Check again", not "Install again".** It
  is already installed; asking again is the repair Locust can perform.

## 0.184.0 - 2026-09-19

- **A hanging CLI leaves nothing behind on Linux and macOS too.** 0.182.0
  said this and it was true only on Windows; a tester counted forty-one
  leftover processes on Linux. Off Windows a version check now runs in its
  own process group and the whole group is ended when it times out.
- **A conversation that could not be read is now said in the sidebar,** not
  only on All missions. Deleting the one visible turn of a chain whose other
  turn was unreadable used to look like the whole conversation was gone.
- **A teammate's own subagents are not teammates.** The brief now says so.
  A Chief of Staff asked to hand work to Booty was spawning its runtime's
  own worker and calling the result Booty's.
- **A Custom role answers with its title** when asked who it is, rather than
  with the name of the program it runs in.
- If the team memory file cannot be written before a run, the failure is now
  recorded in the error log instead of vanishing.
- The first screen's hint no longer says "and sign in to" beside a row that
  needs no account.

## 0.183.0 - 2026-09-18

- **A conversation that left its group keeps its join line.** The thread
  used to mark only where the group's instructions stopped, so the turns
  written under them no longer said so. Both lines are drawn now, and they
  bracket the turns the instructions governed.

## 0.182.0 - 2026-09-18

- **A coding CLI that hangs no longer leaves processes behind.** When one
  did not answer its version check, Locust stopped waiting but the program it
  had started kept running: five more every time it looked again. Locust now
  ends all of it.

## 0.181.0 - 2026-09-18

- **A coding CLI that hangs no longer blocks the first hour.** If something
  on this machine answers to a CLI's name but never replies, its row used to
  read CHECKING forever with no Install anywhere. After Locust has asked
  four times the row now says NOT ANSWERING and offers Install again.
- **The command palette closes on Escape** wherever your focus is, the way
  the plus menu already did.
- The message box no longer asks you to "sign in" when nothing has asked
  for a sign-in.

## 0.180.0 - 2026-09-18

- **Installing a CLI with no Node.js now finishes the job.** 0.178.0 could run
  the install and then not find what it had installed. Three things were
  wrong, and all three were measured on a machine with nothing on it: npm
  put the CLI beside Locust's own program where nothing looked; the npm
  Locust carries refused to run the CLI's own install step; and that step
  needed a program called node, which was not there. Now the CLI lands in a
  folder Locust owns and searches, the install step is allowed for the one
  package you asked for, and a stand-in for node is supplied for the length
  of the install. Press Install on a blank machine and about twenty seconds
  later OpenCode reports its version.
- **Ask mode works on the free model again.** The free OpenCode model had
  begun refusing every run from Ask with "free tier can only be used from
  within OpenCode", because Ask removed the shell tool and the provider no
  longer recognised the client. The shell tool is now offered and every use
  of it is refused by OpenCode itself, so Ask is still read-only and the
  free model answers.
- **Putting back one turn of a conversation puts the conversation back,**
  even if you had opened the other turn to check what was left. Before this
  the sidebar showed two rows for one conversation until you restarted.
- **A single-digit answer is a single digit.** Asking for one used to draw a
  question about whether to answer in complete sentences instead.
- The Install tooltips no longer say the buttons are "below".

## 0.179.0 - 2026-09-18

- **Settings search knows the words you would type.** Searching for *memory*,
  *worktree*, *node* or *ledger* used to say nothing matched, on a screen that
  has all four. They are all found now, and the result names the section they
  live in rather than repeating your word back. *recycle bin* finds the Trash.
- **The page opens where you searched,** instead of at the top. Searching for
  *trash* lands on Trash, not on Updates further up the same page.
- **The note where a group's instructions stop** now sits at the spacing it
  was designed with: clearly separated from the turn above, bound to the turn
  below. It was drifting wider than that in both directions.
- **The first screen no longer points the wrong way.** The line explaining
  why Install is unavailable said the buttons were below it. They are above
  it.

## 0.178.0 - 2026-09-18

- **You no longer need Node.js to install a coding CLI.** Locust carries its
  own copy of npm and runs it with its own binary, so the Install buttons on
  the first screen work on a machine with nothing else on it. Before this
  they were all switched off, on a screen whose entire job is installing
  something — the app that could not install anything was the app you had
  just downloaded in order to install something.
- **The screen says which npm it used.** When Node is absent Locust says the
  install ran on the copy it carries, and says the part that is still true:
  the CLI works inside Locust, and your own terminal will not see it until
  you install Node.js yourself.
- A CLI installed this way **runs without Node too** — Locust hands it the
  same runtime it uses itself.

## 0.177.0 - 2026-09-17

- **The app says what changed.** The first time you open a new version, the
  workroom says so and you can read the entry without leaving the screen.
  It says it once: the version is written down as soon as it is shown.
- **Settings carries it permanently**, under **This app → Updates**, so you
  can look up what this build was at any time.
- **The changelog is public.** It is published to the releases repo with
  every ship, and locust.lol links to it. Before this it existed only in the
  private repo — the one account of what changed that nobody could read.

  The notes ship inside the installer, so what the app tells you was
  packaged with the bytes it is telling you about, and reading it needs no
  network.

## 0.176.0 - 2026-09-17

- **Settings is a list of pages, not one long scroll.** The five areas are
  now pages with a list beside them, the way Claude Code does it: you can
  see the whole map at once and land on any of it in one press. Nothing
  about the settings themselves changed — same sections, same order, same
  explanations behind **How it works**.
- **Search finds a setting by its own name.** Typing *auto* narrows the list
  to the page that holds Auto mode and says so underneath it.

  The areas arrived on 2026-09-14 and were the right grouping in the wrong
  shape: four of the five sat below the fold, and nothing told you the fifth
  was there.

## 0.175.0 - 2026-09-17

- **Deleting a conversation can be undone.** It still leaves every list the
  moment you delete it, which is what deleting means. What changed is that
  the record itself is kept, byte for byte, until you empty the trash —
  under **Settings → Trash**, where each one can be put back whole, owner
  and name included. Emptying is still two presses and still permanent.
- **Bulk retention is undoable too**, because it goes through the same path:
  deleting missions older than N days now fills the trash rather than the
  disk.

  Why: on 2026-09-17 eighteen conversations went in four seconds through the
  Missions screen's select-and-confirm, and the files were unlinked —
  no Recycle Bin, no shadow copy, nothing to undo. A day of portfolio work
  came back only because those runs happened to be on Cursor, which keeps
  its own transcripts. A product whose claim is a durable local record
  cannot lean on another program's copy for that.

## 0.174.0 - 2026-09-17

- **A long conversation title no longer paints over the window.** A
  conversation with no teammate takes its title from your own first line,
  and that line was drawn in full in a 60px header — 380px of it, across the
  title bar and over the header beneath. It is one ellipsised line now. The
  design agent spotted it in a frame; the cause turned out to be the title,
  not the thread.
- **The plus menu closes.** Escape closes it, clicking anywhere else closes
  it, and changing screen closes it. Before this the only ways out were
  choosing a row or pressing **+** again, so the menu sat over Missions,
  Rooms, Routines and Settings while you tried to read them.
- **A disabled Install looks disabled.** OpenCode's Install — the one button
  the first screen is selling — rendered in full lime while doing nothing,
  because the primary colour was declared after the disabled colour.
- **The route chip does not name a route it has not got.** It reads *No
  runtime*, and hovering it used to say *OpenCode / account-default*.
- **Group settings asks for the right kind of instruction.** Its placeholder
  was a finance example; this is a coding-agent control room.
- **Thread rhythm, to the design agent's numbers:** a boundary note now has
  24px above it and keeps 12px below, so it reads as attached to the turns
  it governs and separated from the ones it does not. A turn still opens
  22px down, now expressed as a named gap rather than a bare 10px.

## 0.173.0 - 2026-09-17

- **A Copilot teammate can be sent a long brief too.** The command-line
  ceiling Locust refused at, 8,191 characters, is cmd.exe’s — and Locust
  has launched Copilot past cmd.exe, through node directly, since 0.21.
  Measured: Copilot under node took a 9,228-character prompt and answered.
  The check now follows the launch: the small ceiling only where a run
  really goes through cmd.exe, Windows’ own 32,767 otherwise.
- **A Cursor teammate can be written to on Windows even when the reply
  would have been read-only.** A teammate on Auto lends *Ask* to a
  teammate that has never run, and Cursor Agent cannot be held read-only on
  Windows — so the reply was refused before it started and the exchange
  died at the first hop, with the reason said only on screen. The relay now
  lends *Accept edits* in that case and the thread says why.
- **Measured on your own models.** The same two exchanges that were measured
  on the free model were run on Cursor Grok 4.6 Low: the request carried
  the context, the purpose and what a good answer looks like; the reply
  quoted its evidence and said what was unverified; the chief of staff
  delegated a newcomer's brief with an end state and "do not invent
  limits", and reported back with Answer, Sources and Still open.
- **Measured on Claude Code, live:** a key a teammate repeats is scrubbed
  from the ledger — the raw value never lands, the answer reads
  *[redacted]*.

## 0.172.0 - 2026-09-17

- **Team memory is a file a teammate can read, not only a list it is
  handed.** The brief pasted up to 24 memories on every turn and told the
  teammate the rest existed; on a folder with 33 memories that was 20 pasted
  and 13 invisible, every time. Now Locust writes the whole list to
  `.locust/memory.md` in the workspace before each run (kept out of git
  through `.git/info/exclude`, rewritten only when it changes), pastes the
  newest eight, and tells the teammate to read the file when the task
  touches something remembered. Measured on the free model: with 30
  memories and the answer only in the oldest, the teammate read the file and
  answered from it, naming the file. The memory block is still the only way
  memory changes; the file says so at the top.

## 0.171.0 - 2026-09-17

- **The thread says where a group’s instructions stopped.** Move a
  conversation out of a group, into another, or remove the group, and a
  line appears at the first turn that was not briefed: *Trading’s
  instructions no longer apply from here* — the mirror of the line that
  says where they began. The name and words are kept as they were, so the
  line stays true after the group is renamed, edited or gone.
- **A reply between teammates opens with what is true, not with a
  negation.** One phrase in the reply brief, from Grok Build’s rule; the
  same exchange measured before and after.

## 0.170.0 - 2026-09-17

- **A Custom role is a brief too.** A teammate you gave your own title
  ("release manager", "finance bro") is now briefed around that title in
  the same shape as the presets: do the work that title describes the way
  a capable colleague with it would, bring what someone in that role would
  know, and say plainly when an ask falls outside it. A Custom teammate
  with no title yet gets the shared part alone.

## 0.169.0 - 2026-09-17

- **An OpenCode teammate can be sent a long brief.** OpenCode took its
  prompt on the command line, and Windows stops a command line at 8,191
  characters — so the first real Chief of Staff exchange lost its last hop:
  the report back into your conversation, carrying the teammate’s reply
  quoted inside the standing brief, was refused as too long and you never
  got it. Measured against the CLI: `opencode run` reads the prompt from
  input when none is given on the line, so that is how it is sent now, the
  same way Codex CLI and Claude Code already are. The ceiling no longer
  applies to OpenCode. (Copilot CLI still takes its prompt as an argument.)

## 0.168.0 - 2026-09-17

- **Teammates brief each other the way you would brief a colleague.** A
  message to a teammate used to be asked for as "one or two sentences"; it
  is now asked for as what you need or found and why, the facts by name,
  and what a good answer looks like. A reply is asked for as the answer
  first, then its evidence, then what is still unverified — and ending the
  exchange is named as the normal good outcome, not something to avoid.
  Measured on the same exchange before and after: the request gained its
  reason and an acceptance line, the reply gained its evidence and its
  caveat, and both exchanges still ended in two hops.
- **A role is a brief now, not a label.** Each preset tells the teammate
  what good work in that role looks like, in a sentence or two. And there
  is a new preset, **Chief of Staff**: a teammate whose job is to route your
  ask to the teammate whose role fits, brief them properly, tell you who it
  went to, and report the reply back as one message that stands on its own.
- **The last message stands alone.** Every teammate is told that its last
  message is what you read if you read nothing else: the answer first, then
  what was done, then what is blocked or unverified, said plainly.
- **Cursor connectors work in every mode.** Outside Auto, Cursor asked
  before every connector call and nobody was there to answer, so each one
  failed as *user rejected*. Locust now writes an allow rule per configured
  server into the workspace’s `.cursor/cli.json` before the run starts;
  nothing is ever removed and the deny list is never touched. Measured:
  the same read-only call was rejected in Accept edits before and answered
  after. The mode menu says so.

## 0.167.0 - 2026-09-17

- **When a run stops because the ledger could not be written, the card says
  why — and the reason is kept.** Wembley’s run stopped this morning with that
  card and nothing on the machine could say what had failed: the error was
  caught and dropped. Three different failures also shared that one card —
  the ledger refusing a write, the runtime adapter choking on a record, and
  a follow-up write — and all three were blamed on the ledger. They are told
  apart now, the card carries the error’s own words, and
  `locust-errors.log` gets a line.

## 0.166.0 - 2026-09-17

- **A thought is one line.** *Thought for 12s*, with a thought-bubble beside
  it, and the words folded underneath until you open them — the shape
  Claude Code used. The thinking is all still kept and still one click away;
  it just stops taking the whole screen to say it happened.

## 0.165.0 - 2026-09-17

- **A key a teammate repeats is scrubbed from what the ledger keeps — in
  its answer and in its thinking, on every runtime.** It was scrubbed from
  the evidence and not from the text: a live Cursor turn given a fake `sk-`
  key wrote `[redacted]` in one field and the key verbatim in the answer.
  Every message text now goes through the same scrub at the point it is
  bounded. (What *you* typed is still recorded as you typed it.)
- **Long Codex answers keep their second half.** Separating the scrub from
  the size limit found that a Codex answer over 8,192 characters was being
  cut in the ledger at the evidence limit after already being bounded at
  the message limit — the thread showed all of it, the record kept half.

## 0.164.0 - 2026-09-17

- **A handoff keeps your effort level too.** The third place the level was
  dropped on the way to disk: handing a run to another runtime remembered the
  teammate’s new runtime, model and mode and not the effort. Found by Grok
  reading source after an unsigned Codex would not start. Fixed.

## 0.163.0 - 2026-09-16

- **A group’s menu is three items again.** *Group settings…*, *Rename*,
  *Remove group* — with a hairline before the destructive one. Its standing
  instructions and default route moved into one **Group settings** dialog,
  because a menu holds actions and a dialog holds what a thing carries. The
  route is named the way the composer names it — *OpenCode / Muse Spark 1.3*,
  not a model id — with *Use current* and a *Clear* that is disabled rather
  than missing when there is nothing to clear. The counter shows only past
  3,600 characters. (The design agent’s ruling on the 0.162.0 frames.)
- **Two shorter sentences.** The thread’s line reads *Trading’s instructions
  brief every turn from here*, and its edit pointer names the dialog. The
  sidebar’s unreadable-file notice is now the fact, then the reassurance
  underneath in small type, behind an amber rule — so it stops reading as a
  row you could click.

## 0.162.0 - 2026-09-16

- **Your effort level stays put.** The level you chose was dropped twice on
  the way to disk — the app remembered a teammate’s runtime, model and mode
  and not the fourth field — so selecting the teammate restored three and
  reset the effort every time. Both places keep it now.
- **Opening a conversation puts the composer back on its mode.** A
  conversation you last ran on Auto reopened on whatever mode the previous
  screen left — so the stocks conversation that needs Auto for its
  connectors quietly came back on Accept edits, and the next connector call
  failed. The conversation’s last turn recorded its mode; the composer now
  starts there. And the guard that knocks Auto down when Auto is switched
  off waits until it actually knows, instead of firing in the second before
  settings load.
- **The mode menu says what Cursor does with connectors outside Auto.**
  Cursor asks before each connector call, and in a Locust run nobody is
  there to answer, so each one fails as “user rejected” — 17 in one
  Accept-edits run, 0 on Auto, same teammate, same connector. The menu says
  so before the run does: use Auto for connector work.

## 0.161.0 - 2026-09-16

- **A group can carry a default route.** Right-click a group · *Use
  current route as default* records the runtime, model, mode and effort
  the composer is set to right now, and says so on the control. When you
  move the conversation on screen into that group, the composer switches
  to the group’s route for the next turn — and only the next turn: nothing
  about a turn already run changes, and a conversation you are not looking
  at is never touched. *Clear default route* takes it away.

## 0.160.0 - 2026-09-16

- **The thread says where a group’s instructions began.** A conversation
  in a group with standing instructions now carries one line at the point
  it joined — *“Trading’s standing instructions brief every turn from here
  · view”* — after the last turn that ran without them, never at the top,
  because the turns above genuinely were not briefed. *view* shows the
  group’s words read-only; editing goes through the group’s own header, so
  nobody changes shared text believing it is their own. Conversations filed
  before the app recorded join times get no line rather than a guessed one.

## 0.159.0 - 2026-09-16

- **Groups carry standing instructions.** Right-click a group · *Add
  instructions…* and write what every conversation in it should be told —
  "quote sizes in shares", "analysis only, never propose a trade". Every
  turn started from then on in a conversation in that group is briefed with
  them, after the folder’s own LOCUST.md and before memory. Turns already
  run were not briefed, and nothing claims they were. Clear the text and the
  group is a plain folder again. A group is no longer only a place to file
  things: filing a conversation into one buys something.

## 0.158.0 - 2026-09-16

- **When a teammate’s reply lands back in your conversation, the teammate
  you asked now tells you what it means.** Ask Jimothy to get a brief from
  Wembley and get back to you: the brief came back, and Jimothy — briefed
  that nobody was watching — ended with a note to itself and not one word
  to you. The reply that lands in the conversation you started is now told
  that you read it, and to say in a line or two what the answer was, where
  it is, and what is still open. Replies between two teammates are still
  told to end quietly, so exchanges do not run to the budget.

## 0.157.0 - 2026-09-16

- **A local file that cannot be read is never treated as empty — and never
  written over.** This morning’s groups fix turned out to be one of four:
  your teammates, rooms and routines files had the same habit. The worst
  was the teammate roster: if it could not be read for a moment (a lock, a
  scan, a torn byte), the app showed you a fresh install, and the next
  mission start would have saved that empty roster over the real one. Now
  every one of those files is either read or refused, nothing is saved over
  a refused one, and the sidebar names the files that would not read.

## 0.156.0 - 2026-09-16

- **Thinking is not counted as a tool call — on the line you actually read.**
  0.153.0 fixed the count in one place and missed the header of the fold,
  so two reads and a thought still said `3 tool calls`, and a run that only
  thought said `1 tool call`. Found by Grok on 0.154.0.
- **The filter count is the pile you are standing in.** With a teammate’s
  face selected and a search typed, the line said `1 of 20` — the whole
  folder — next to a list of that teammate’s one match. It now says `1 of
  5` when five is what that teammate has. Search alone still counts against
  the whole folder. Found by Grok.
- **On a machine with nothing installed, the message box no longer tells
  you to sign in.** There is nothing to sign in to; it says install a coding
  agent, and keeps “and sign in” for a runtime that is present and signed
  out. Found by Grok on the original bare-machine route.

## 0.155.0 - 2026-09-16

- **A file summary counts each file once.** When a teammate changed one
  file in several steps, the run’s card added the steps together and then
  added the file’s final change on top — two lines changed in two edits read
  as `+3 −3`, over a file git reported as two and two. Now the final change
  to a file is the count, and the steps that led to it are inside it. Found
  by Astra on 0.154.0.
- **If your groups file cannot be read, the sidebar says so.** It used to
  look exactly like having no groups: every conversation listed ungrouped
  and not a word about why. Nothing is lost when this happens — nothing is
  saved over the file until it reads again — and now the sidebar tells you,
  instead of leaving you to wonder where your groups went. A file cut off
  part-way now counts as unreadable too — it used to read as empty, and the
  next save would have made it so. Found by Astra, who replaced the file with
  a directory, a truncated copy and an oversize one, and got the same silent
  sidebar all three times.

## 0.154.0 - 2026-09-16

- **A conversation keeps the name you gave it, and stays in the group you put
  it in.** If you renamed one or moved it into a group while it was still
  running, both could come undone on their own a little later — the name
  reverting to your first sentence and the conversation quietly leaving its
  group. Names and groups you set before this will come back; nothing was
  lost, it was being filed under the turn that was live at the time instead of
  under the conversation.

## 0.153.0 - 2026-09-16

- **Thinking no longer breaks up the list of tool calls.** Yesterday's change
  put a teammate's reasoning in the run's fold, and it landed in the middle of
  the tool calls — so it was counted as one, listed among their names, and
  split a single run of calls into several. A turn that made five calls said
  six, and a fold that should have been one line became three. Thinking has
  its own row now, and the count is of tool calls again.

## 0.152.0 - 2026-09-16

- **You can read what a teammate was thinking.** Reasoning was thrown away
  before it was recorded — Cursor's was replaced with "[redacted]" and
  Codex's was stripped alongside API keys and passwords — so a run that
  thought for a minute could tell you how long and nothing about what. It is
  kept now, and appears in the run's fold beside the tool calls. Secrets are
  still removed from it.
- A note on what that means: your mission records will be larger, and a
  record you send to someone now carries the model's working-out as well as
  its answer.
- **Telling a teammate something need not start a paid run.** A share marked
  `when="later"` waits for their next mission instead of starting one. (The
  models are not told about it yet — see below.)

## 0.151.0 - 2026-09-15

- **A mission is no longer stopped by a file that was busy for a moment.**
  "The mission ledger could not be written" could fire because something else
  on the machine — a virus scanner, an indexer, a backup agent — held the
  file for a few milliseconds while it was being written to. Locust waits and
  tries again now. A real failure, like a full disk, still stops the run at
  once, and a failure part-way through a write still stops it, because there
  the record really is uncertain.

## 0.150.0 - 2026-09-15

- **Move a conversation into a group from its own menu.** Right-click it and
  **Move to group** opens the list, with a tick on the one it is already in,
  **Ungrouped** to take it out, and **New group…** at the bottom — which makes
  the group and puts that conversation in it.
- **A teammate is told which folder it is working in.** That was only ever
  said inside a folder's `LOCUST.md`, so on a machine without one — every new
  install — a coding teammate was told its name, its role and its colleagues,
  and nothing about which folder it was about to edit.
- The teammate faces show five when the team is five or fewer, and four plus
  a count when it is larger, so the count never crowds the row.

## 0.149.0 - 2026-09-15

- **A plan now shows what happened to it.** Every plan was drawn as a plain
  numbered list — no ticks, no `2 of 3 done` — which is right for a turn that
  only planned and wrong for one that carried the plan out. So a finished
  plan looked exactly like an untouched one, while the fold underneath said
  "3 of 3 steps".

## 0.148.0 - 2026-09-15

- **The `+` menu opens properly in a normal window.** It was being cut off at
  the edge of the sidebar, and its three items could not be clicked at all.
- **The sidebar says what it is filtering, in one line.** Picking a teammate
  and typing in the search box are two filters, and only one of them was
  mentioned — so a short list had an unexplained reason. It now reads
  `Atlas · "invoice" — 1 of 5`, with one **Clear** that undoes both.

## 0.147.0 - 2026-09-15

- **The teammate faces stay inside the sidebar.** With a big team the row
  ran off the edge of the column — at twelve teammates the last face was
  drawn well outside it. It shows the five you have worked with most
  recently and counts the rest, and the count opens the roster.

## 0.146.0 - 2026-09-15

- **Groups.** Make one from the `+` beside the logo, then right-click any
  conversation and choose **Move to** it. Groups sit at the top of the
  sidebar, folded, with everything else under **Ungrouped**, newest first.
- The `+` now offers the same three things in any window size — New teammate,
  New room, New group.
- **Removing a group never removes conversations.** They go back to
  Ungrouped. Rename or remove one from the `⋯` on its header.

## 0.145.0 - 2026-09-15

- **A conversation you renamed keeps its name.** Renaming saved it correctly
  and then never read it back when the app started, so the name lasted until
  you next opened Locust and the row went back to your first sentence. It
  loads now, and names you set before this will reappear — they were on disk
  the whole time.

## 0.144.0 - 2026-09-15

- **You can delete more than one mission at a time.** Tick the box beside any
  row on the Missions screen and a bar appears: how many are selected, Clear,
  Select all, and Delete. It asks once more before the records go.
- A mission that is still running has no box to tick, and says why — deleting
  one would take away the control that stops it.

## 0.143.0 - 2026-09-15

- **If a deleted conversation ever comes back, Locust now says so.** It
  remembers what you deleted while the app is open and writes a line to the
  log if one is read back out of the record. Nothing found so far — deleting
  a conversation does remove every one of its turns from the ledger, checked
  by driving it — so this is here to settle it the next time it happens, in
  either direction: a line means a record really returned, and no line means
  the row you are looking at is a different conversation that reads the same.
  Settings · This app · Report a problem shows the log.

## 0.142.0 - 2026-09-15

- **Hovering a teammate shows their card again, not a sentence.** Pointing at
  a face in the sidebar drew one long tooltip — name, role, runtime and an
  instruction, all on a single line. It opens the profile card instead, which
  already existed for the narrow window and says the same things laid out,
  with their conversations under them.
- **Their faces move again.** The new roster row was drawn without the state
  that animates them, so the whole team sat still.
- **The dot beside a conversation means something now.** It showed the run's
  outcome, and *completed* was blue — so nearly every row carried a permanent
  coloured dot that reads like an unread mark and never clears. A finished
  conversation has no dot; one that is running, failed or left an incomplete
  record still does.
- **The sidebar footer stopped cutting its own labels.** It said "Teamma…"
  and "6 connect…". Teammates has left the footer — the faces row is the way
  to the roster — and the runtimes line now says its whole sentence.
- **A running step names the command, not the shell.** It read
  `cmd /c "dir /s /b …"`, spending the front of every row on the same nine
  characters and pushing the actual command off the end.

## 0.141.0 - 2026-09-15

- **You can name a conversation.** Right-click one in the sidebar and choose
  **Rename**. Until now its title was whatever your first sentence happened to
  be, which is fine to type and hard to find a week later — and since the
  sidebar flattened, that title is the only thing on the row.
- **Clearing the name gives you the original back.** Empty the box and press
  Enter. Nothing was overwritten to lose: the words you typed are in the
  mission's own record and renaming never touches it.

## 0.140.0 - 2026-09-15

- **Rooms and Routines are back.** Flattening the sidebar in 0.139.0 left
  them with nowhere to be: in a normal window there was no button, no
  heading, and a room you had already made was drawn nowhere at all. They are
  in the footer now, beside Missions, Teammates and Settings — and in the
  narrow window too, where Rooms had never been reachable.
- **The roster button says Teammates again.** It was shortened to "Team"
  because three labelled buttons could not share 266px without the word
  breaking. The footer is two rows now, so it fits.

## 0.139.0 - 2026-09-15

- **The sidebar lists your conversations, newest first.** They used to sit
  folded under whichever teammate owned them, so finding one meant first
  remembering who you gave it to. On a light week — fourteen conversations
  over four days — that layout spent **330px before the first conversation**,
  nearly half the column, and showed **7 of 14**. It now spends 4px and shows
  all fourteen.
- **Conversation titles are no longer cut short before they reach the
  screen.** The title was trimmed to 44 characters in code and then trimmed
  again by the column, so widening it could never have helped. The column
  does the trimming now, and rows say `2m` / `4h` / `3d` so the order you are
  looking at is one you can check.
- **Your teammates are a row of faces under the search box.** Click one to
  see only their conversations; **Team** opens the full roster, which is where
  the runtime and model belong — that line used to be repeated, and truncated,
  once per teammate.
- **The newest conversation is now actually at the top.** The one the app
  reopens for you had no timestamp of its own, so it sorted to the bottom of
  a list that says it is newest-first.
- The narrow window is unchanged: at 64px the avatars and their flyout are
  still the way to a conversation.

## 0.138.0 - 2026-09-15

- **There is a way to say "this broke".** Settings · This app now has
  **Report a problem**: it names the log, says how big it is, says what is in
  it, and shows it in your file manager. The log records what happened, never
  what was said — crashes, a window that stopped answering, and the version
  you were on. No messages, no file contents, nothing from a mission.
- **A crash that takes the window now leaves a record.** Locust has always
  logged an error it threw itself, but the window dying is a different thing
  and it wrote nothing at all — the app would vanish and leave no trace of
  why. It writes a line now, as does a helper process that dies and a window
  that stops answering. The log is capped and rolls over once.
- **A teammate on a machine with no coding agent stopped asking you to sign
  in.** It said *Runtime sign-in required*, in red, about software that was
  not installed — an instruction you could not follow, on the first screen
  you see after naming a teammate. It now says what is actually true.
- **The loading screen stopped counting nothing.** With no coding agents
  installed it flashed "checking 0 of 0 runtimes", which is arithmetic about
  an empty set. It says what it is doing instead.

## 0.137.0 - 2026-09-14

- **The narrow sidebar works now.** Both of yesterday's complaints are
  closed, this time after driving the built app and measuring the controls
  rather than shipping and hoping. The locust mark at the top is the way
  home again — it had been rendering two pixels wide and no pixels tall, so
  there was nothing to click. And the single `+` opens: it was drawing two
  pixels tall, then, once it had height, getting clipped at the edge of the
  64px rail, which left half of "New teammate" and "New room" unpressable.

## 0.136.0 - 2026-09-14

- **A plan no longer claims nothing changed when something did.** Showing the
  plan on every turn brought its "Plan mode — nothing was changed" line with
  it, over runs that had just made thirty tool calls. The plan stays visible;
  that sentence appears only on a turn that really did nothing but plan.
- **Clicking the message box no longer tints it.** It lightens, the way
  Claude Code's does — the border comes up a step and the ground barely
  lifts. It still says where the keyboard is.
- **The narrow sidebar is back to how it was.** Yesterday's attempt at it
  stretched the mark across the top and stopped the `+` working. Both of the
  original complaints — no way home, and two pluses you cannot tell apart —
  are still open and will be done with a way to see them first.

## 0.135.0 - 2026-09-14

- **Settings is organised.** It was thirteen subjects in one unbroken scroll,
  in the order they happened to get built — Updates third, the two appearance
  settings at opposite ends, and the boot screen sharing a block with Swarm
  and Auto mode because they were added the same day. Five areas now, each
  named for what is in it: **Your workspace**, **Runtimes**, **How teammates
  work**, **Appearance**, **This app**.
  - The explanations have not moved or grown. The problem was never that a
    setting explained itself; it was that you could not find the setting.

## 0.134.0 - 2026-09-14

- **The loading screen is quick on a machine without the runtimes.** Checking
  spaced each runtime a beat apart so the log could be read, and that beat
  was being paid by the people with nothing to check — a runtime that is not
  installed answers in about ten milliseconds, so the spacing *was* the whole
  wait. Each check now waits for the one before it or for the beat, whichever
  comes first: slow runtimes still arrive one at a time, and an empty machine
  goes straight through.
- **The narrow sidebar keeps the Locust mark**, and it is still the way home.
  Both it and the wordmark were hidden, so the app lost its own name at
  exactly the width where the window is smallest.
- **The plan is visible without opening anything.** It used to ride inside
  the tool-call fold whenever a turn had one — so on exactly the turns worth
  watching, the plan was the thing you had to go looking for. The fold hides
  *how* a turn was carried out; a plan is *what* it is doing and how far
  along, which is the question you actually have while it runs.
- **Clicking the message box glows instead of outlining.** The hard edge was
  too loud for the biggest control in the app. Same colour, a fraction of the
  weight — it still says where the keyboard is, which is the part that
  matters.
- **One `+` in the narrow sidebar, and it says what it adds.** There were two
  stacked, unlabelled and indistinguishable; it is now a single control
  offering New teammate or New room.

## 0.133.0 - 2026-09-14

- **The black square behind the loading screen is gone.** Making the window
  transparent was not enough: the page drawn on it painted its own
  background, so the window was see-through and its contents were not. The
  loading screen is now the monitor and its own edge, on your desktop.
- **A conversation's turn count keeps clear of its menu.** The `…` that
  appears on hover sits over the right edge of the row, and the count was
  underneath it — so the number and the dots shared the same spot. The row
  reserves that space now, rather than the count trying to dodge it.

## 0.132.0 - 2026-09-14

- **The loading screen is just the monitor now.** It had a black slab behind
  it and around its corners; the window is transparent, so the only thing on
  screen is the bezel and its own edge.
- **The app comes forward when it opens.** Held behind the loading screen, it
  could arrive minimised behind whatever you were last looking at. It is
  restored, shown and focused — and the loading screen raises itself too,
  since one that opens behind another window is one nobody sees.

## 0.131.0 - 2026-09-14

- **Locust opens on a loading screen.** The monitor is its own window now: it
  comes up first, shows each runtime being checked, and closes when they have
  answered — and the workspace behind it is already built and already knows
  what it found. It is not drawn inside the app any more, which is why it
  used to flash and vanish.
  - If a runtime never answers, the app opens anyway after twelve seconds
    rather than leaving you on a loading screen forever. Clicking the screen
    also ends it early.
  - **Settings → The boot screen → Off** skips it entirely.
- **Teammates keep a plan by default.** It was off unless you found the
  setting, and a settings file written before the setting existed counted as
  "off" rather than "never asked".
- **"New conversation with…" starts one.** It re-selected a teammate who was
  already selected and opened the conversation that was already open, so it
  did nothing at all.
- **The turn count beside a conversation is legible**, instead of running
  into the title's ellipsis.

## 0.130.0 - 2026-09-14

- **The app stops going backwards while it is running.** Locust re-checks
  your runtimes every so often and whenever you focus the window, and each
  check was throwing away what it already knew — so a runtime that had
  reported its version dropped back to "checking" with no version, and the
  welcome panel flickered. A check that has not answered yet now keeps what
  the last one established. Signing out, or uninstalling, still shows at
  once: those are answers, not silence.
- **The boot screen belongs to the launch.** A re-check could bring it back
  over your work half a minute into a session. It appears once.

## 0.129.0 - 2026-09-14

Three from a design pass over the screenshots — each one visible in a frame
and invisible in the source.

- **The elapsed counter no longer runs backwards.** A turn showed `starting ·
  3s` and then `working · 0s`, because the clock restarted whenever the phase
  changed. It runs from the start of the turn now and the phase is a label on
  it. That number is how you tell a slow runtime from a stuck one, so it must
  only ever climb.
- **The command palette prints each heading once.** `GO TO` appeared twice,
  because Workroom sat in that group with an unrelated row between it and the
  rest. Workroom now has its own heading at the top — it is first because it
  is the likeliest thing you want, and now it looks deliberate.
- **You can actually watch the boot screen now.** It was starting before the
  window appeared and running faster than the window takes to open, so by the
  time you could see it the log had already happened. The runtimes are not
  checked until there is a window to watch it in, and they start far enough
  apart to read.
- **The Skip button is gone.** There was nothing to skip; clicking anywhere
  still puts the screen away.
- **The boot screen counts what it shows.** It said "8 so far" above six
  rows, because it was counting runtimes that are not built yet and so are
  not listed.
- **Settings lists what you can use first.** Two runtimes that are not built
  yet were sitting in the middle of six that are, so the list had to be read
  tag by tag. Ready, then experimental, then needs-sign-in, then not
  installed, then roadmap — alphabetical inside each, so it does not shuffle
  between launches.

## 0.128.0 - 2026-09-14

Four fixes to yesterday's boot screen, all from watching it run.

- **A slow runtime no longer looks like a broken one.** Cursor's command is
  `cursor-agent` but its name inside Locust is `cursor`, and the screen was
  filing the answer under the wrong one — so it sat on "still waiting" for a
  runtime that had already replied, and then showed it connected in the table
  underneath. The wording is calmer too: a slow answer is not a refusal.
- **The log and the table are the same list now.** Gemini was in the log
  saying "sign-in required" while Settings called it not built yet; it is
  roadmap, so it is out. Antigravity was in the table having never appeared
  in the log, because it is checked differently; it is in.
- **Starting up is quick again.** Checking one runtime at a time made the
  wait the sum of all of them, and on a machine where they are all installed
  that is many seconds. They now start a beat apart and run alongside each
  other: the log still reads in order, without the slowest one holding up
  everybody else.
- **A teammate's message reads in their voice wherever it is quoted.** A
  message from one teammate shown inside another's conversation was set in
  the interface font while their replies were not.
- **The jump-to-bottom button sits where it should**, instead of wherever the
  layout happened to put it, and now reads as floating above the text rather
  than punched into it.

## 0.127.0 - 2026-09-14

- **Finding your runtimes is something you can watch now.** Locust opens on a
  terminal that shows what it is actually doing: the version and folder it
  opened, then each runtime as its command is issued, with a counter running
  while that one is out. When they have all answered it settles into the
  table of what you have. Nothing on it is invented — every line is something
  the app read.
  - It only ever fills the empty middle of the window. The sidebar, the
    folder and the message box stay put, so you can pick a teammate or start
    typing without waiting for it, and clicking anywhere on it puts it away.
  - If discovery is quick you get a flash and you are straight in. If a
    runtime takes more than six seconds the counter stops pretending and says
    it has not answered, and a Skip appears.
  - **Settings → The boot screen** turns it down to Subtle or off entirely.
- **Runtimes are now checked one at a time** instead of all at once, so the
  log reads in the order things happened.
- **A queued message is no longer drawn like an error.** It was a bordered
  card with two heavy buttons for the most ordinary thing in the app —
  something you typed that will send by itself in a moment.

## 0.126.0 - 2026-09-14

- **Text no longer bleeds across the composer's controls.** The `effort ·
  fixed` label and the swarm mark were drawn on top of each other — measured
  at 44px of overlap in a default window and 83px in a small one. That row is
  capped and deliberately does not wrap, and the rule that lets the folder
  and model names truncate was letting everything else shrink below its own
  text too. Only those two names give now.
- **The conversation's running total is out of that row.** 0.125.0 put it
  there for runtimes that draw no context ring, and that row was already at
  capacity. It stays in the ring's hover where a ring exists; where to put it
  otherwise is back with the design review rather than answered in the
  tightest row in the app.

## 0.125.0 - 2026-09-14

From a static sweep of the whole renderer.

- **Tabbing into a text box now shows you where you are.** The search field,
  the composer, the command palette and the route picker each hid their own
  focus ring and put nothing back, so moving between them by keyboard
  changed nothing on screen. The palette is keyboard-only by nature, which
  made it the worst of the four.
- **A one-step plan no longer reads "0 of 1 steps".**
- **The count beside TEAMMATES and ROOMS is readable.** It was using the tone
  reserved for things that carry no information, at about 3.2:1.
- Violet and clay teammate names now come from the colour system like every
  other hue, and no colour in the stylesheet can quietly fall back to a value
  that was never checked.

## 0.124.0 - 2026-09-14

Two findings from the first acceptance pass to reach routines.

- **A routine held for a question now says what running it again would do.**
  It said "Answer it, then run the routine again when you are ready", which
  reads as *carry on from here*. It isn't: the attempt is held and cannot be
  continued, and a later Run starts from step 1. If your steps are not safe
  to repeat, that is the sentence you needed before you pressed anything.
- **The sidebar search says it searches conversations**, because that is
  what the sidebar holds. It said "Search missions" while the screen actually
  titled Missions carried on showing everything — so from that screen, search
  looked broken.

## 0.123.0 - 2026-09-14

Three placements, from a design review of yesterday's fixes. Each fix was
right and each left some surface saying one thing too many.

- **The mission line fits again.** It had grown to seven facts and was
  truncating mid-word. Off it: the word `Mission`, which labelled the line
  once and cost eight characters on every render after; the permission
  sentence, which the composer already states where you can actually change
  it; and the conversation's running total, which is not a fact about one
  mission. What is left is the mission, its model, its state and what it
  cost.
- **The conversation's total moved to the route chip**, where the context
  ring already tracks what this conversation has spent — in its hover where
  a ring is drawn, and stated outright where there is none.
- **"Started without the earlier messages" now reads as a note**, not a
  divider — left-ruled beside the turn it describes rather than centred like
  a boundary between turns.
- **A message from a teammate says who *sent* it**, and offers to open the
  conversation it was written in. What arrives is what a teammate chose to
  send; their own reply may be fuller, and now you can go and read it.

## 0.122.0 - 2026-09-14

- **A run that never started offers to run again.** When a runtime dies
  before it opens a session — nothing started, no tool called, nothing
  touched — the card now offers one press instead of leaving you to retype
  the message you already wrote. It is offered *only* in that case: a run
  that got as far as doing something is deliberately not offered it, because
  whether the half it did matters is your call and not the app's.

## 0.121.0 - 2026-09-14

- **"Started without the earlier messages" now sits where the turn starts.**
  It was drawn at the very bottom of the thread, after the approvals and hard
  against the composer — which is where live and pending things live, so a
  permanent note about the past read as an alert that would not go away. It
  is neither an alert nor dismissable: it says this turn began without the
  model carrying the conversation, because the turn before it left no session
  to resume. Beside the time marker that opens the turn, it reads as what it
  is.

## 0.120.0 - 2026-09-14

- **A runtime that cannot save its own settings file now says so in English.**
  Asking one Cursor teammate to ask another could end the run with two
  absolute paths, a uuid and "EPERM: operation not permitted" — which reads
  like Locust was refused something in your project. It wasn't: `cursor-agent`
  rewrites its own config in your home folder each time it starts, and on
  Windows that fails while a second copy of it still has the file open, which
  is exactly what a message between two Cursor teammates causes. The card now
  says whose file it was, that nothing in your workspace was denied, and that
  running it again usually works. It deliberately does not claim your
  workspace is untouched — the run died somewhere, and nothing here knows
  where.

## 0.119.0 - 2026-09-14

A second outside audit drove the same build again and re-ran every original
repro rather than the new happy path. Seven of the eight earlier findings held
up. These are the three that did not.

- **The teammate you named claims the row you named.** A post like "Wren,
  start Write the release notes" recorded Wren's run without ever reaching the
  claim, so the board said unassigned for the whole time she was visibly
  working it — and the claim only landed when her reply came back, which is
  what the previous build already did. The claim now happens where the run
  actually starts. It goes to the teammate the sentence names, not to whoever
  starts first: on a post that also says "Booty, reply OK and do not touch the
  board", first-past-the-post would have handed Booty the row. A post that
  names no row still claims nothing.
- **A cancelled or failed run no longer wears the previous turn's tokens.** A
  run stopped before anything started showed "1.8k in · 189 out" beside its
  own id — the counts from the turn before it, which its own record does not
  contain. The number beside a mission is now that mission's, or nothing at
  all when it never reached a model. The conversation's running total is still
  there and now says the word "conversation".
- **A link the app refuses to open says so.** Locust declines addresses it
  won't hand to your browser, and it had a sentence ready for each one — which
  the window threw away. A refused link and a link that opened looked
  identical: nothing happened either way. The reason now appears beside the
  link you pressed.

## 0.118.0 - 2026-09-14

Seven findings from an outside audit that drove the built app, all of them on
screen rather than read out of the source.

- **A rename that cannot be saved no longer says the room does not exist.** If
  the rooms file could not be read, the app read zero rooms, decided your room
  was not among them, and said so — under a window that was showing it. It now
  says nothing was changed and every room is as it was.
- **An Ask-mode answer with a code block is not a failed write.** Asking for an
  example put a banner under the reply saying the run could not write to your
  workspace. Nothing had tried to.
- **A room task is claimed when the work starts**, where your own words named
  the row, instead of only once a reply comes back. A post that doesn't name a
  row still claims nothing — the app doesn't guess which one you meant.
- **A stopped run that never started drops the warning about a command still
  running.** One card said nothing had started and that something may still be
  finishing.
- **The Team roster shows the same presence as the sidebar.** One of them
  counted a starting mission as working and the other didn't.
- **With nothing installed, the composer says "No runtime"** instead of naming
  one that isn't there.
- **A room menu closes when you leave the room**, instead of floating over the
  next screen still offering to remove it.
- **OpenCode's session error reads as a sentence**, not as the provider's raw
  `encrypted_content` line.

## 0.117.0 - 2026-09-14

- **A Cursor teammate knows which connectors it can call, by name.** You should
  not have to tell it. If a machine has a working connector and a broken one
  with a similar name, a teammate asked about it would find the obvious name,
  read "0 tools", and report the whole thing dead — which was true of the one
  it checked and false of the one beside it. Only connectors that are actually
  ready are named.
- **Two amber lines are gone.** A teammate addressing a message to its own role
  said so in amber; nothing was lost, nobody was waiting, and there was nothing
  to do about it. A memory a teammate successfully forgot was quoted in full
  above the card that already said it. Amber here means a person may need to
  act, and neither of those did. A message to a name that is not on the roster
  still speaks, and so does a forget that failed — those are things somebody
  has to settle.

## 0.116.0 - 2026-09-14

- **A teammate claims the task it starts.** Ask one to do something a row on
  the board describes and it now claims that row in the same reply. Every
  instruction it had was a limit — *claim only what you are actually doing* —
  and a model reading only limits errs toward doing nothing, so the board said
  "unassigned" while someone was actively working it. That is not untidiness:
  the rule beside it sends other teammates at unassigned rows.
- **One control per task, not four.** Open, Assign, Done and Remove were four
  boxed buttons on every row, so a five-task board drew twenty of them and the
  tasks were the quietest thing in their own list. They are all one press away
  now, in the same menu the room header uses.

## 0.115.0 - 2026-09-14

- **A room's header is a conversation's header.** It was a boxed Rooms button,
  the title, a list of names and a boxed Remove room, all competing across the
  top. It is now the room's name with its members under it, and one dropdown on
  the right holding All rooms, Rename and Remove — the same shape a teammate
  conversation has.
- **The plan step underway pulses.** It already had the colour and the border,
  and it was the only motionless thing in a view where motion means "happening
  now", so on a six-step plan nothing drew your eye to the row that was
  actually moving. It uses the same pulse the sidebar uses for a working
  teammate, and it stops entirely if your system asks for reduced motion.

## 0.114.0 - 2026-09-14

- **Links in a reply are clickable, and they say where they go.** They were
  shown but never linked. A web address now opens in your own browser, with
  the site's host printed quietly after the label — because the label is the
  model's words and the destination is the fact, and you should see the second
  one before you click. A local file path is still not clickable: a reply must
  not become a way to reach this machine.
- **An italicised link renders as a link.** `*[Title](https://…)*` — which
  models write constantly for an article title — used to render with the
  brackets and the whole URL sitting in the middle of the sentence.
- **The Cursor connector notice is gone from the conversation.** It said to run
  `cursor-agent mcp login`, and that command does not work: it persists no
  credential anywhere. An app that prescribes a command which cannot work
  spends your evening for you.

## 0.113.0 - 2026-09-14

- **A teammate whose runtime is signed out no longer says "idle".** If you made
  a Claude teammate on a machine where Claude is not signed in, the sidebar
  said "idle" while Settings said, correctly and in red, that Claude is not
  signed in. It now says "Runtime sign-in required" everywhere, with the red
  dot, on the sidebar, the Team roster and the workroom header.
- **Their welcome screen says it too, with the command.** It used to offer
  starter buttons that could not work. It now says which runtime is signed out
  and what to run, and the buttons are disabled rather than misleading.
- **A mission waiting on your approval is still its own thing.** Amber, and
  worded as a decision waiting for you — not folded in with a sign-in wall,
  which is the app's problem rather than yours.

## 0.112.0 - 2026-09-14

- **A review sees the whole conversation, not just the last turn.** If a
  teammate created a file on turn two and adjusted it on turn four, a review of
  turn four was shown one path and told that was the work — so a reviewer doing
  its job properly reported the work as incomplete. Files changed earlier now
  travel with the brief, listed separately so the reviewer can still tell which
  turn it is judging.
- **This matters most for a turn that only answered.** In a conversation that
  had already built something, the brief used to say "it changed no files and
  ran no commands" and stop, which is exactly where a reviewer concludes the
  work was never done.

## 0.111.0 - 2026-09-14

- **A teammate that fails to forget something says so.** When a teammate
  corrected itself, it quoted the memory it was replacing — and if the quote
  was not word-for-word, nothing was removed and nobody was told. The wrong
  memory stayed in every brief, the correction landed beside it, and both were
  read by every mission afterwards. It now says what it could not forget, and
  that the memory is still there.
- **Quoting a memory no longer has to be exact.** Dropping a trailing clause or
  quoting half the line finds it. What it will not do is guess: if the quote
  could mean two different memories, it removes neither and names both, so you
  can see what it was reaching for.
- **Memories say how old they are.** A note from three weeks ago read exactly
  like one written an hour ago. Each line in a teammate's brief now carries its
  age, and teammates are told that when two notes disagree the newer one is
  usually the correction.

## 0.110.0 - 2026-09-14

- **Ask your teammates to keep a plan.** New in Settings, under Plans, off by
  default. Switch it on and a Codex, Cursor or OpenCode teammate is asked to
  keep a todo list as it works, so the board in a room fills in while the
  mission runs instead of after it. It costs tokens and changes how a teammate
  narrates itself, which is why it is a choice rather than the default.
- **Claude Code is never asked.** It has no todo tool at all, so the request
  would be an instruction it cannot follow and the board would sit empty with
  nothing to explain it. The switch does nothing for a Claude Code teammate in
  either position, and says so.

## 0.109.0 - 2026-09-14

- **The Cursor connector notice actually appears now.** It shipped yesterday
  and had never once run on Windows: Cursor installs `cursor-agent` as a `.cmd`
  shim, and the way Locust was starting it cannot start a `.cmd` at all. The
  reading failed instantly every time and the app treated that as "nothing to
  report". It now finds the launcher the same way it finds every other runtime.
- **A Cursor mission no longer waits on that reading.** The same call sat in
  the middle of the event stream, so with it fixed every event behind it would
  have queued up behind a question asked of another program. It answers in its
  own time now.
- **Settings tells you how to sign a runtime in.** A red SIGN IN tag there said
  nothing else at all, while the first-run panel had been printing the exact
  command since it was built. The screen you open when something is wrong knew
  less than the one you see once.
- **The teammate dot means the same thing everywhere.** The workroom header
  drew its own and had no "blocked" state at all, so a teammate stuck on a
  sign-in looked identical to one with nothing to do — inches from a sidebar
  row drawing it correctly. The Team roster had no dot at all.
- **A mode says one thing.** The five modes were described by four separate
  tables in four files, already disagreeing on capitalisation and on whether
  Ask is read-only, which is the most important fact about it. One table now.
- **Every failure says what is still true.** "That room could not be renamed."
  tells you about the request and nothing about the room, and the reading
  people assume is the one that would hurt. Twenty-three of these now name what
  did not change: the room kept its name, the mission is still running, your
  message is untouched.

## 0.108.0 - 2026-09-14

- **The room's plan sits with the conversation, not above it.** It was pinned
  at the top, which meant that in any room with a conversation in it the board
  was scrolled off screen exactly when you would act on it. It now sits under
  the newest messages, where you already are.
- **Adding a task is a line, not a bar.** A bordered full-width input used to
  be the loudest thing in an empty room, for the least important thing in it.
  A plan has steps and a quiet way to add one.
- **Locust says when a Cursor connector has no credential.** If you have a
  connector set up but a Cursor teammate never calls it, this is almost always
  why: `cursor-agent mcp list` reports `requires_authentication`, and signing
  in inside the Cursor app does not sign the CLI in. The app now says so, with
  the exact command that fixes it, once per run.

## 0.107.0 - 2026-09-14

- **"no word back yet" is gone from the conversation.** Added yesterday for a
  Cursor run that sat silent for two minutes, it fired on almost every Cursor
  run instead — noise where it was meant to be signal. Rooms still say it,
  where it has always been on a card rather than under a working line.
- **A waiting message says when it was sent.** A teammate's message waits for
  its recipient's next turn, and that turn can be a conversation about
  something else — so a paragraph about a database schema can open a
  conversation you started to ask about something unrelated. Nothing was wrong,
  and the card never said the one fact that made it make sense. It does now.
- **One route, one spelling.** The Missions row and the Team roster printed raw
  ids, so `opencode / muse-spark-1.3-contributor-free` on one screen was
  `OpenCode / Muse Spark 1.3 Contributor Free` on another.
- **One missing cost, one sentence.** Four surfaces each answered "the runtime
  reported no cost" differently — `—`, `not reported`, and two longer versions
  — with three of them in view at once.

## 0.106.0 - 2026-09-14

- **A 41-second run says 41s everywhere.** The Missions row rounded to whole
  minutes and read `0m` while the fold two inches below said `41s` — one
  mission, timed twice, disagreeing.
- **A completed run whose record is incomplete is amber wherever it is drawn.**
  The Team screen's recent list had no way to know, so it stayed blue while the
  same mission went amber on every other surface.

## 0.105.0 - 2026-09-14

- **Stopping a run now stops what the run started.** Measured on the installed
  app: a teammate was asked for one command that writes a file, waits ninety
  seconds, then writes another — and stopped after eighteen. Both files were
  there afterwards. The command had kept running and written into the workspace
  a minute and a half after the person stopped it. The stop reached the CLI and
  never the command underneath it. It does now, and the case is a test that
  costs nothing to run.

## 0.104.0 - 2026-09-14

- **Nested lists are nested.** Every item became a top-level row whatever its
  indent, so a three-level answer came out as one column of equal-weight lines
  — and in a list, the structure *is* the content. Depth now comes from the
  indent itself, so two spaces, four spaces and tabs all draw the same shape,
  and a numbered list written under a bullet is a child of it rather than a new
  list.
- A leading `/` is a command only where a person typed it — pinned as a guard,
  so a teammate's message or a routine step beginning with `/model` can never
  become one.

## 0.103.0 - 2026-09-14

- **A run of plain tool calls is one row.** A turn that read eleven files drew
  eleven rows of equal weight, and the one edit among them looked exactly like
  the ten reads. Reads, searches and lists that happen in a row now fold into a
  single quiet line. A command, a file change, anything still running and
  anything that **failed** always keeps its own row — the fold can only ever
  quieten what was already quiet.
- **That row says what it is not showing.** It names the first few and then
  counts the rest (`… 8 more`), rather than stopping without saying it stopped.
- **Conversations are ordered by what is happening, not by what started last.**
  A conversation working for an hour sat below one that opened five minutes ago
  and had been idle since.

## 0.102.0 - 2026-09-14

- **A stopped run no longer claims nothing was in flight.** It said "Nothing
  was mid-flight" from a list that only holds tool calls the runtime had
  reported as open — while, measured on a real run, the command it had launched
  kept going and wrote a file into the workspace ninety seconds after the stop.
  It now says what it actually knows, and says out loud that a command which
  had already started may still finish on its own. The stop itself not reaching
  that command is a separate fault and is not fixed yet.
- **A review is asked about the turn you are reviewing.** In a conversation
  that had moved on, the reviewer was handed the request the conversation
  *opened* with — so a turn asked to create two files was judged against an
  earlier instruction not to change any files, and correct work read as a
  violation. The request is now this turn's own, with the opening ask kept
  beside it and labelled as context.

## 0.101.0 - 2026-09-13

- **Everything you type while a teammate works is kept, and arrives as one
  instruction.** There was one queue slot, so a second thought *replaced* the
  first and you watched your own words disappear. The queue is a list now, and
  the run of plain follow-ups at the front is folded into a single turn when it
  goes — so a teammate answers all of them knowing they exist, instead of
  answering the first without knowing the other two were coming. The strip says
  how many are waiting and shows the instruction that will actually be sent.
- Anything the app queued for you — a routine's next step, a decision reply, a
  hand-off — never merges into your words, and keeps its own turn.

## 0.100.0 - 2026-09-13

- **A teammate's words can no longer act as another teammate's instructions.**
  Locust reads four blocks out of a reply and each makes the app *do*
  something — send a message, write to the team's memory, raise a decision,
  move a room's board. Text one teammate said gets quoted into another's
  prompt in several places, and a model asked to review or reply to quoted
  text often reproduces it verbatim — at which point the block is parsed out
  of the *second* teammate's reply and acted on under their name. Peer
  messages have been defended against this since they existed, but only for
  the share block; the reviewer brief added in 0.96.0 defended against none.
  All four tags are now defanged by one rule, everywhere model text is
  re-quoted, and the words still read normally.

## 0.99.1 - 2026-09-13

- **A finished task is muted, not crossed out.** Checked against the design
  artifact itself: its done steps are quieter and nothing more, and the app's
  own plan agrees. The board was the only one of the three drawing a line
  through them.

## 0.99.0 - 2026-09-13

- **The room's task board is a plan.** Its rows already borrowed the plan's
  shape; everything around them did not — a bordered panel with a titled head,
  above every room, drawn at full size even with nothing on it. The card is
  gone. What is left is the plan's own quiet counter, `TASKS · 1 of 3 done`,
  steps with room to breathe, and no rules between them. A board with no tasks
  now draws **nothing at all** except one faint line to add the first one, so a
  new room opens on the room rather than on an empty panel.
- **Adding a task stopped shouting.** With the rows quiet, a full-strength
  input box became the loudest thing on the board — the bar rebuilt one element
  down. It sits back until you hover or focus it.
- **A teammate a post never reached is no longer called a failure.** The line
  above a post read "2 asked · 1 answered · Booty failed" directly above "Booty
  was not asked." Nothing ran, so nothing failed; they are counted as not
  asked, which the line underneath already explains.

## 0.98.0 - 2026-09-13

- **Cursor's plan is drawn.** Cursor keeps a to-do list and Locust threw every
  update away — on the runtime that accounts for 90 of the 147 missions
  recorded here. It now shows as the same live step list every other runtime
  gets. Cursor sends only the items that *changed* after the first update, so
  the plan is merged by item rather than replaced; without that, a three-step
  plan would shrink to one as it finished.
- **Antigravity's commands are counted as commands.** It calls them
  `run_command`, which nothing recognised, so an Antigravity run that ran
  seventy-six commands reported that it ran none — in the activity fold, in the
  trace line, and in what a reviewer is handed.
- **Reading a file no longer counts as changing one.** Antigravity's
  `view_file` was read as an edit because it contains the word "file", so runs
  reported changed files they had only looked at. A verb like *view* or *read*
  now beats the noun.
- Every tool name five runtimes have actually produced is now pinned in a test
  built from the recorded ledgers, so the next runtime's spelling has to be
  measured before it is trusted.

## 0.97.0 - 2026-09-13

- **A run that has not said anything now says so.** A Cursor turn showed
  "working ···" and nothing else for over two minutes. That was the truth —
  measured in its own ledger, the runtime sent nothing for 128 seconds and then
  thirty events inside three — but "working" reads the same whether a run is
  thinking or hung. Past twenty seconds of silence the line adds **no word back
  yet**, which is the sentence rooms have carried since 0.89 and the
  conversation never did.
- **A memory is announced once.** Keeping one drew an amber line quoting it in
  full *and*, directly underneath, the card holding the same sentence. The card
  is the only surface for it now, and it carries the "keep or forget it on the
  Memory screen" action on its face. A memory that was **forgotten** still gets
  a line, because a card read from the memories that exist cannot draw one that
  is gone.

## 0.96.0 - 2026-09-13

- **A reviewer is shown what the teammate SAID.** Asking for a review of a
  piece of research got back "That did not happen… the work is missing
  entirely" — about a full page of analysis that was on screen the whole time.
  The brief carried the files a turn changed and the commands it ran and never
  its reply, so work whose deliverable is an answer looked like nothing at all.
  The reply now leads the brief, a turn that changed nothing says so as a fact
  about the kind of work rather than as two absences, and the reviewer is asked
  about the work rather than about "the change".
- **OpenCode's plan shows its progress.** It keeps a three-step plan and
  updates it as it goes; every update was drawn as one more `todowrite done`
  row, so the plan advanced four times and no surface moved. It is now the same
  live step list every other runtime gets. A run with no plan still gets none.
- **Every chat follows the newest line, and offers a way back down.** The
  conversation already followed; rooms did not, so a post to several teammates
  grew under you. And nothing anywhere offered the way back once you had
  scrolled up to read — there is now a small control for it, for exactly as
  long as you are away from the bottom.
- **A room says who is still working.** It could read "2 asked · all answered"
  while both teammates were still running, because they had spoken. Speaking is
  not finishing, and the line now says so.
- **A file in a room reads the same as it does in the conversation** —
  `README.md`, not its whole absolute path.
- **The permissions panel stops explaining Codex to other runtimes.** The
  footer describing `codex exec` approvals appeared under every runtime's
  allow-list, including runs it had nothing to do with.

## 0.95.0 - 2026-09-13

- **Tables are drawn as columns.** Teammates write them constantly — a quarter
  against a quarter, one name against another — and the thread printed the
  pipes. They now come out as a real table, with the figures lined up under
  each other and its own sideways scroll when it is wider than the
  conversation.
- **`---` is a divider and `>` is a quote**, rather than three hyphens and a
  chevron.
- **A reply is set in Anthropic Serif.** Anthropic publish the family and its
  own guidance is that the serif carries body copy while the sans is for
  chrome — so the teammate's voice is the serif, and the app keeps Geist and
  Geist Mono for everything that is the app talking. The whole of a reply
  wears it now, not only its paragraphs: lists, headings, tables and quotes
  had been falling back to the app's own face mid-answer.

## 0.94.0 - 2026-09-13

- **The ".cursorignore hides this folder" warning only appears when a read is
  actually refused.** It used to greet every Cursor run in such a folder,
  whether or not that run ever opened a file — true, inapplicable, and
  unavoidable, an amber line above every exchange. It now waits for the
  refusal it explains and arrives beside it, once per conversation. A teammate
  that only searches the web never sees it at all.

## 0.93.0 - 2026-09-13

- **The permissions panel told you the opposite of the truth on an Auto run.**
  It called a full-access run "workspace-write" and listed "deny: anything
  outside the workspace" — which is exactly what Auto allows — while the
  conversation header two inches away said "may edit anything on this
  machine". It now has three states instead of two, takes its wording from the
  same function the header uses, and says plainly that an Auto run may write
  anywhere and run any command your account can.
- **Six internal checks were measuring nothing.** They looked for parts of the
  screen that had been removed, and passed because the old styling was still
  in the stylesheet. The check that was meant to prevent that counted a
  leftover style rule as proof the app could still draw it; it now only counts
  what a component actually renders.

## 0.92.0 - 2026-09-13

- **A teammate's whole turn stays in the room.** Their progress appeared as
  they wrote it and then vanished the moment they finished — the room kept
  only the last message, so three quarters of what was said disappeared at the
  end. Opening the same conversation showed all of it. Everything they said is
  now in the room, in order.
- **The task board is a plan, not a dashboard.** Every row carried a state
  tag, the text, an owner, and four buttons — six things competing on one line
  for a list of two. It reads like the plan card now: a tick or a dot for the
  state, the text struck through when it is done, the owner as one quiet note,
  and the same "1 of 2 done" the plan uses. Assign, Done and Remove are still
  there and appear when you reach for a row.

## 0.91.2 - 2026-09-13

- **The ".cursorignore hides this folder" warning is said once per folder, not
  on every run.** It is a standing fact about your machine — as true on the
  tenth mission as the first, and it cannot change while the app is open
  without you editing the file it names. Repeated, it became one amber line
  per teammate per post and the loudest thing in a room, about something you
  had already read. It still says the rule, the file and the fix, the first
  time a folder needs it.

## 0.91.1 - 2026-09-13

- **A room no longer fills with "Unhandled Cursor record" lines.** Cursor
  announces a web search or a web fetch on two channels, and Locust already
  draws one of them as a proper tool row — the other was being reported, once
  per record, as something it did not understand. Two dozen identical amber
  lines could bury a conversation and made a working teammate look broken.
  The duplicate is ignored; the search still shows up where it belongs, in the
  turn's work.
- And when Locust really does meet a record it does not know, it says so
  **once** for that run rather than once per record. That a version sends a
  kind we do not read is a fact about the stream, and repeating it does not
  make it truer.

## 0.91.0 - 2026-09-13

- **A teammate now reads your question last, not first.** The brief was built
  the wrong way round: your words at the top, then the roster, the memory and
  the block formats — so the last thing a teammate read before answering was
  boilerplate rather than what you asked. It now leads with the standing
  things and ends with the question.
- **That should also make missions cheaper.** Providers reuse a cached copy of
  a prompt's opening when it does not change; ours changed at the front on
  every turn, so none of it could be reused. The opening is now identical from
  turn to turn — about 1,700 characters of it, more when memory is fuller.
- Plan mode is the one exception and stays as it was: its "change nothing this
  turn" instruction is still the last thing read, because a promise the app has
  to keep outranks a general rule about ordering.

## 0.90.0 - 2026-09-13

- **Your team's memory no longer takes over every brief.** Every memory the
  app had was sent to every teammate on every turn. Measured on a real store:
  56 memories came to about 3,700 tokens, against 900 characters for
  everything else — so most of what a teammate read before your actual
  question was old notes, and you paid for it on every turn of every mission.
  It grows, too: the app keeps up to 400.
- A brief now carries the most useful two dozen — this folder's before
  everywhere's, newest first — and **says how many it left out**, so a
  teammate that needs an older one can ask rather than quietly working from a
  shortened list. The same conversation went from ~3,700 tokens of preamble to
  ~1,500.
- Nothing is forgotten. Every memory is still kept, still on the Memory
  screen, still editable.

## 0.89.0 - 2026-09-13

- **A room shows the whole of what a teammate did, in the chat.** Each
  teammate's answer was a little card in a grid showing their LAST message and
  nothing else — so a teammate who said three things showed one, and their
  thinking, tool calls and plan were not drawn at all. That is the "missing"
  part you could glimpse while it loaded. A room now renders the same thing
  the conversation does, one per teammate: every message, the work fold with
  its commands and files, the plan and how far through it is, and the live
  line while it is still going.
- **No boxes.** A teammate talking is not a card anywhere else in the app, and
  it is not one here. They stack down the page in the order they replied —
  whoever answered first is first — under a quiet rule that groups each
  teammate's turn.
- Side by side was also what forced the summary: a 280px column cannot hold a
  work fold and four messages. Stacked, each teammate gets the width the
  conversation has.

## 0.88.1 - 2026-09-12

- **The receipt says what it verified.** It ended with a green `verified`
  beside `COMPLETED`, which reads as an endorsement of the work. It has never
  meant that — it means the record of the mission is whole and recovers — so
  it now says `ledger verified`, the same words the Missions screen has always
  used for the same fact. A finished turn that did the wrong thing can have a
  perfectly intact record, and nothing on this screen should suggest otherwise.

## 0.88.0 - 2026-09-12

- **Conversations recorded before the doubling was fixed now read correctly
  too.** 0.85.0 stopped a Cursor reply saying itself twice, but every
  conversation already on disk still held the doubled text — and a record is
  only ever added to, never rewritten. So the reader repairs it instead: a
  passage that restates exactly what the message has already said is that
  message ending, not twice as much of it, and whatever follows is the next
  reply rather than more of the same one.
- Measured on a real conversation: one 2,872-character block that began by
  saying its first sentence twice becomes the 88-character note it started
  with and the 2,696-character answer that had been stuck to the end of it.
  Nothing on disk changed.

## 0.87.1 - 2026-09-12

- **Fixes a serious bug in 0.87.0: a Cursor run in a folder `.cursorignore`
  hides recorded nothing at all.** The warning 0.87.0 added was written into
  the mission's record, and the record requires its events to be numbered
  without gaps — so the warning took a number the run was going to use, and
  everything the run did afterwards was refused. Two teammates would do real
  work and be reported as having written nothing back, and the conversation
  ended with "the mission ledger could not be written". The warning is now
  shown without being recorded, which is also the truer place for it: it is a
  fact about this machine right now, not about what the mission did.
- **And the warning's advice made sense.** It could tell you to change a rule
  into itself. When the rule is already narrowed it now says to add the one
  line that lets the folder back in.

## 0.87.0 - 2026-09-12

- **A Cursor teammate now says when it has been told not to read the folder.**
  Cursor obeys `.cursorignore`, and its tools refuse an ignored file with
  "permission denied" and no reason — so the teammate invents a reason, and
  the invention is what you read. It happened three times on one machine, the
  last of them to a screenshot Locust itself had just written into the
  workspace it then handed over. Locust reads the rule first and names it:
  which file, which line, and what to change it to.
- Nothing is changed on your behalf — the rule is yours, and the folders these
  rules cover are usually excluded for a good reason.

## 0.86.0 - 2026-09-12

- **The app tells you where your connectors live.** Locust reads your
  connectors from Claude Code, and only a teammate on a Claude Code route gets
  them — which it had never said anywhere. On any other route the mode menu
  now names that plainly. It does not claim the other route has none: Cursor
  has its own, signed in separately. That silence cost a whole evening of
  chasing a Robinhood connector that was signed in on Claude Code and could
  never have worked on Cursor.
- **A room's task board says whose each task is, from your teammate's side.**
  A row read `(Yurt)`, which left every teammate to work out whether that name
  was their own — so both members of a two-person room started the same task.
  Rows now read `(yours)`, `(Yurt's)` or `(unassigned)`, and a teammate is
  told plainly that somebody else's row is not theirs to take.

## 0.85.0 - 2026-09-11

- **A Cursor reply no longer says itself twice.** Cursor streams a reply word
  by word and then sends the whole message again to mark it complete. Locust
  recognised that closing message by a field Cursor has stopped always
  sending — so it was appended as if it were new text, and the reply read
  "…hand the numbers to Yurt for his take.Looking up NVIDIA forward
  earnings…". It is now recognised by what it says, which is not something a
  version can stop sending.
- **And a teammate who wrote a reply is no longer reported as having written
  nothing.** That was the same bug, and the worse half of it: the message was
  never marked finished, and everything the app reads out of a reply — a
  message to another teammate, something remembered, a task claimed, a post
  to the room — reads the finished one. So a reply that was plainly on screen
  could be announced as "the runtime finished and wrote nothing back".
- Separate replies in one turn are also kept apart again, instead of being
  run together into a single block of text.
- **The line under a room's message box is quiet now.** It narrated the task
  board back at you — "Jimothy took on X. Jimothy finished X." — directly
  under a board already showing exactly that, and when it had no news it fell
  back to a standing sentence. It now speaks only when a post is in flight or
  something was refused.

## 0.84.0 - 2026-09-11

- **Your Cursor connectors work in Locust.** A connector configured in
  `~/.cursor/mcp.json` is approved in Cursor by answering a prompt — and a
  Locust run has nobody to answer it, so the prompt was resolving to "no" and
  every connector call failed. Measured against a real configured server:
  `Failed: user rejected MCP`. That is a rejection, not a sign-in problem,
  which is why the same connector worked in Cursor's own app and not here.
- Locust adds no connector of its own and reads no credential. What a
  teammate can reach is exactly what their CLI was already set up to reach.

## 0.83.0 - 2026-09-11

- **Text is no longer eaten from the beginning while a teammate is writing.**
  0.82.0 fixed this for a conversation you reopen; this fixes it as it
  happens. The same limit existed in two places, and the second one is the
  one you watch: a long reply pushed its own beginning out of view while it
  was still being written, then snapped back to the whole thing when the run
  finished. Both places now join a reply back together as it arrives.
- **Room posts are shown whole.** The fold past twelve lines and its
  "Show the rest" are gone — a room is where teammates argue in front of you,
  and the argument is the content.
- **Routes read like names.** `Cursor Agent / cursor-grok-4.6` is now
  `Cursor / Grok 4.6`, in the composer, the sidebar and the conversation
  header. The exact model id is still in the receipt and in every tooltip,
  which is where you go for a string to copy.
- **One control at the top of a conversation instead of four.** Ask for a
  review, Save as routine and Delete moved into a `⋯` menu; Activity stayed
  out of it, because it toggles a panel. Delete still asks before it acts,
  in the menu, exactly as it did as a button.

## 0.82.0 - 2026-09-11

- **A long reply is no longer cut off at the front.** Reopening a conversation
  could show an answer that began mid-sentence and looked complete — no
  ellipsis, nothing to say anything was missing. Measured on one real
  conversation: the record held 4,853 characters and 1,954 reached the screen.
  Every character is there now.
- Why it happened, because it is worth knowing: a reply arrives in hundreds of
  small pieces, and the limit on how much of a conversation gets loaded was
  counting those pieces as if each were a separate thing that happened. It kept
  the most recent few hundred, which is the END of the reply. The pieces are
  joined back into the message before that limit is applied.
- Long conversations also open faster, because one reply is now one thing to
  load instead of a thousand.

## 0.81.0 - 2026-09-11

- **You find out an exchange stopped, even if you were looking elsewhere.**
  "Stopped after 12 automatic replies" reached the window and nowhere else, so
  anyone watching a different conversation when it fired never learned — and
  reopening the thread later showed nothing, so the exchange simply appeared
  to stop for no reason. Endings are written into the mission's own record
  now, and are there when you come back.
- Only **endings**. "Still waiting on Booty" is true for a moment and false
  after it, and a record of it would be a record of something that is no
  longer so.

## 0.80.0 - 2026-09-11

- **The reply budget counts what it always claimed to.** It counted the depth
  of one chain, and an exchange is not one chain — a reply held for a busy
  teammate starts later on its own branch, so every decision could be inside
  the budget while the total was outside it. Measured: seven automatic runs
  against a budget of six. It counts the whole exchange now, and never less
  than the chain already proves.
- **And the budget is a backstop again, not a timer.** It was six, and six was
  firing as the ordinary way an exchange ended — five runs of a one-word
  question went 6, 6, 3, 6, 7. Now that teammates actually stop when they are
  done, it is twelve by default and up to twenty-four, which you should never
  meet while the work is real.
- A meeting's reply-back also respected a built-in number rather than the one
  you chose in Settings. It respects yours.

## 0.79.0 - 2026-09-11

- **Teammates stop thanking each other.** An exchange is meant to end when a
  reply has nothing more to say; measured across five runs of a question whose
  answer is one word, it ran to the budget four times out of five. The brief
  told a replying teammate "write back only if that helps finish the work",
  which a polite model reads as permission. It now says the plain fact: the
  other end is a **model, not a person**, every reply starts another whole
  billed mission, and thanking, confirming receipt or summarising what you
  both agreed reaches nobody and costs a run each.
- A relayed brief also says where in the budget it is — *"This is automatic
  reply 3 of 6"* — and on the last one, that anything sent back waits for a
  person rather than reaching them, which is what actually happens.

## 0.78.0 - 2026-09-11

- **Ask another teammate to review a finished mission.** A new action on any
  completed conversation hands it to a teammate of your choosing, with what
  you asked for, what changed, what ran and its exit codes, and where it ran.
  A teammate cannot see another's conversation, so the facts have to travel.
- The brief **makes no claim that the work is done or correct** — telling a
  reviewer it passed and then asking it to check is handing it the answer. It
  also says plainly: report, do not fix; and if the work is fine, say so
  and stop.
- Tested with the answer hidden: a builder was asked for two files, then told
  in a follow-up to delete one and not mention it. The reviewer is shown only
  the original request, and found it — *"notes.test.md was never created, the
  only recorded action was deleting it."*

## 0.77.0 - 2026-09-11

- **The context ring is back, beside the route it belongs to.** It had moved
  to the mission header, where a run whose runtime reports no context window
  left nothing at all — so it read as gone. It sits immediately left of the
  model picker now, and in one place only.
- **The attach `+` is a `+` again, not a chip.** The other controls on that
  row are modes — permission, folder, route, effort — and a chip is the right
  shape for something that shows what is currently set. Attaching a file sets
  nothing.
- **A turn no longer grades its own commands.** `ran 4 commands · all exit 0`
  shipped yesterday and was the one place on that line where the app
  aggregated four facts into a verdict — "all" plus "0" is about as close to
  *passed* as you get without typing it, and a reader who took it that way was
  not misreading. It names them instead: `ran pnpm check, tsc and 2 more`. A
  command that failed is unchanged: that one is news, and it stays amber.
- **The receipt says what a run ran ON.** `Ran on Windows · locust-astra`,
  beside the runtime it already names. It is the one thing in this space
  knowable with certainty — a run happens on one machine, in one environment —
  and it tells a person who changed a path handler everything a "this did not
  test macOS" warning would have, while telling someone who changed a copy
  string nothing, which is correct.

## 0.76.0 - 2026-09-11

- **"Build this plan" no longer sometimes starts a stranger.** Clicking it
  while the plan run was still settling recorded the build turn with no link
  back, so it began a fresh conversation instead of following the plan it was
  offered from. Intermittent by construction — a run's last event lands on the
  next frame, so for a few tens of milliseconds the window had not heard that
  the mission was over. The host settles it now, and settles it stricter.

## 0.75.0 - 2026-09-11

- **A finished turn says what it ran, not only what it changed.** The line
  read `3 files` or `no files changed` and never mentioned the commands — so a
  run that edited three files and a run that edited three files and proved
  them looked identical. It now reads `ran 3 commands · all exit 0`, or names
  the one command when there was only one. A command that exited non-zero, or
  that never reported at all, is said in amber.
- Deliberately **no guess about what a command means**. Nothing here decides
  that `pnpm test` is a test and `ls` is not; it says what ran and what came
  back, and you decide whether that is evidence. An app guessing at proof
  would be worse than one staying quiet.

## 0.74.0 - 2026-09-11

- **The effort slider agreed with nothing.** It read `EFFORT high`, put the
  knob hard left against "Faster", and said "slower, costlier" underneath —
  three statements about one value. Cursor does not list its models low to
  high (`cursor-grok-4.6-high-fast` comes before `cursor-grok-4.6-low`), and
  the scale was built in listing order, so `high` landed at position one. The
  scale is sorted now, and a level this build has never seen still keeps the
  runtime's own order.
- **A Cursor route stated the wrong effort, and ran at it.** A teammate saved
  on `cursor-grok-4.6-high` read as `medium` — the effort in the model name
  was dropped and a default put in its place. The name says which level it is;
  the app now reads it.
- **"Already has a mission running" no longer eats your message.** Sending a
  moment too early marked the turn failed, drew a red "The run could not
  continue", and threw away what you typed. It is not a failure — it is "not
  yet" — so the message waits behind the run in front of it and goes when that
  one finishes, which is what sending it meant.
- **Changing the folder no longer looks like a crash.** It reopens Locust —
  every service binds its folder at start-up — and that sentence was behind a
  fold nobody opens, with the window vanishing before the "Reopening in …"
  notice could be read. Both fixed.

## 0.73.0 - 2026-09-11

- **A reply comes down the thread instead of lurching.** Measured on a real
  400-word answer: the thread used to move on 26 of 1361 frames, with 47% of
  the whole journey in five of them and one single jump of 221px — a third of
  a screen at once. It now walks: 123 moving frames, a biggest jump of 17px,
  and 9% in the worst five. The text is on screen the moment it arrives; the
  page just takes a few frames to walk to it. Opening a conversation still
  jumps, and "reduce motion" still snaps.
- **A room shows what a teammate is doing, not just that they are.** A live
  turn in a room drew a flat "Gem is replying…" while the thread two clicks
  away animated and named the tool. It is the same line now, in both places —
  the register, the animated dots, the tool or connector, and the clock.

## 0.72.1 - 2026-09-11

- **A connector call is named as one even without the `mcp__` prefix.** A
  Google Drive call read as `using a tool · Google_Drive__create_file` — an
  ordinary tool with a strange name. Every connector name this app had been
  shown carried the prefix, so the split required one; this one does not.
  Ordinary tools are untouched: none of Read, Write, Bash, Grep, Task or
  WebFetch carries a double underscore, which is the whole safety of the rule.

## 0.72.0 - 2026-09-11

- **The Missions screen shows what is happening now.** It listed only recorded
  missions, and a mission reaches the record when it finishes — so the one
  screen whose job is "what is going on" was the last place to hear about it.
  Measured: for 49 straight samples a teammate was visibly working in the
  sidebar and this screen was empty. Live runs are listed as they run.
- **A live mission says RUNNING.** It wore INTERRUPTED, which is the right
  reading of a record with no ending and the wrong word for a run still going.
  The screen knew; the row never asked.
- **Each row says what it is doing, or what it wants.** Under the title:
  `using a tool · Read` while it works, `Pending: …` when it stopped to ask
  you something, and nothing at all once it has settled — so a list of twelve
  tells you which one to open without opening them.

## 0.71.0 - 2026-09-11

- **A teammate can ask to be taken now.** A message that arrives while its
  recipient is working normally waits for their run to end — usually right,
  sometimes far too late, because the message worth interrupting for is "stop,
  I'm editing that file". A sender can now mark one urgent, and with the new
  Settings switch on, that stops the recipient part-way so the message is
  their next one. Off until you turn it on: it throws away whatever they had
  in flight, and their unfinished work stays in their own conversation. The
  budget and the replies switch still bound everything — an interruption only
  ever shortens a wait, it is never a second way to start work.
- **A long tool step no longer swamps the live line.** Claude Code packs a
  subagent's type, its description and its last tool into one message, and a
  search pattern in the description wrapped the line onto two rows. It is one
  line again, and the runtime's word for the kind of step is gone — the
  register already says it, in words you'd use.

## 0.70.0 - 2026-09-11

- **You can tell your teammate talking from the app narrating.** Every live
  line now says whose it is and which register it is — `Jimothy · thinking`,
  `Jimothy · using a tool`, `Jimothy · using a connector · Robinhood` — and
  that word is derived from what the run is actually doing, never from what
  the runtime called it. A connector is named as one, because it is the one
  kind of call that reaches off this machine.
- **A tool call shows while it is happening, not only after.** Claude Code
  reports no step for a tool call, so a run that spent thirty seconds reading
  files just said "working"; the open tool is now what the line says, and it
  names the file or the connector it is working on.
- **A room stops going quiet mid-argument.** A reply the host is starting is
  drawn as the turn it is, and so is a turn that is running but has not spoken
  yet. Before, both were invisible for as long as a cold runtime takes to
  boot, which reads as the conversation being over — and the budget line no
  longer says "post again to continue" while a reply is on its way.
- **Posting to a room is immediate.** The box empties when you press Post and
  the post appears at once, instead of holding your words under a "Posting…"
  until every teammate has been asked. If the post genuinely fails, your words
  come back.
- **The room's top bar has a shape.** Back on the left, the room's name with
  its members on a line under it, Remove room alone on the right — instead of
  three unrelated things clumped together in the middle.

## 0.69.0 - 2026-09-11

- **Two teammates can now actually argue.** If you post to a room and both
  answer at once, each one's message used to arrive while the other was still
  working — and a teammate takes one mission at a time, so the relay gave up
  and the argument died before it started. A message that lands mid-run is now
  held and delivered the moment that run ends. The room says so while it
  waits: *is part-way through another mission. Their reply starts when it
  ends.*
- **The room shows the whole argument, not half of it.** A post starts one
  mission per member, and a reply is a new conversation of its own, so an
  argument that began on both sides at once was two halves — and the room drew
  whichever half was bigger, silently dropping somebody's opening. It now
  draws every turn, in the order they were said.
- **A teammate reads the same in a room as in a thread.** A room drew literal
  `**asterisks**` and hyphens where the mission thread drew bold text and a
  list; it now uses the thread's own reader. And a turn no longer opens with a
  gap where a stripped share block used to be.
- **Posting to a room uses the same box as everywhere else.** The wide filled
  *Post* button under the field is gone; the field and the round send control
  are the ones you already know from a mission.

## 0.68.0 - 2026-09-11

- **A teammate runs on the route you gave it, even in the first seconds after
  launch.** While Locust was still finding your runtimes, a message sent
  straight away could start on whichever one happened to be ready first — a
  teammate set to Codex CLI running on OpenCode, and failing. Its own saved
  route decides now. Changing the model in the bar still wins, and a
  conversation with no teammate is unchanged.

## 0.67.0 - 2026-09-11

- **A conversation in "approve each action" continues.** A reply used to
  start over with no memory of the turn before it — ask a teammate to
  remember a word and it could not tell you the word. It keeps the thread now,
  like every other mode.
- **Routines, room posts and relayed messages can use "approve each action".**
  All three used to refuse it outright, because the cards could not be shown
  from those paths. They run it, and the cards appear.
- Under the hood the mode stopped being a separate machine and became an
  ordinary mode of the one that runs everything else, which is what made both
  of the above possible.

## 0.66.1 - 2026-09-10

- **Approve-each conversations continue.** A second message to a teammate in
  Approve-each was refused with "The approval-capable runtime could not be
  started" while the sidebar showed the teammate idle: a finished turn was
  never let go. Four of them would have filled the pool and refused every
  mission until a restart. A finished turn releases its run now. The reply
  still starts without the earlier turn's memory on this mode; that is next.
- **What a runtime says about itself is no longer dressed as a command's
  output.** Notices like "Skill descriptions were shortened" sit in the fold's
  footer behind a hairline, labelled with who said it, and in the quiet tone:
  amber is for things you can act on, and there is nothing to act on there.
- Reply prose is capped at 90 characters of its own face with a little more
  leading; the receipt's value column and tool rows no longer stretch across
  a wide window.
- In the sidebar, the role never takes the state's colour, and a conversation
  row inside a selected card no longer gets a second highlight.

## 0.66.0 - 2026-09-10

- **Replies fit the window they are read in.** A reply used to sit at a fixed
  width in the app's smallest type, using a little over half the column it
  already had. It now fills that column, at a larger size meant for reading
  rather than scanning, and on windows wider than about 1650px the column
  itself grows with the window.
- **Stop works on the second turn of a conversation.** Pressing stop while a
  turn was still starting did nothing at all, silently, and the mission ran to
  completion. It is now remembered and takes effect the moment the run can be
  named. A first turn always worked, which is why this went unnoticed.

## 0.65.1 - 2026-09-10

- **The home screen is addressed to nobody again.** The row of teammate
  chips is gone from the composer, and with it the teammate it used to tick
  for you before you had picked anyone. Write and send from the home screen
  and you get a plain conversation on the model the bar shows; hand it to a
  teammate afterwards with "Assign to ..." on its row, or pick a teammate in
  the sidebar first and message them directly.
- Picking several teammates at once lives in rooms, where it always did:
  "New room" in the sidebar, and the room's form picks its members.

## 0.65.0 - 2026-09-10

- **Codex replies stream now, in every mode.** A Codex teammate used to
  think in silence and then drop the whole reply in one paint; only
  Approve-each streamed. Ask, Accept edits, Plan and Auto now run over the
  same transport Approve-each always used, and the reply arrives as it is
  written — measured 2 paints before, 33 after, on the same prompt.
- A Codex run's header shows what it cost again (`40k in · 275 out`), and
  counts up while it runs. Approve-each never showed this and does now.
- Follow-ups on Codex keep the earlier turns, as before.

## 0.64.0 - 2026-09-10

- **A teammate can be limited to some of your connectors.** Open a teammate
  and under "Works in" there is now a row of your connectors, all on. Untick
  one and that teammate is limited to the rest: a Finance Bro gets Robinhood
  and not Gmail. A call to anything outside its list stops and asks you, the
  same card as before. Nothing ticked means everything, as it always did.
- The refusal a teammate reads after you deny a connector no longer claims
  there was no way to ask you.

## 0.63.1 - 2026-09-10

- **Settings → Connectors: "ask before every connector call".** Off, a
  teammate uses any connector your Claude Code can reach without asking, as
  before. On, every connector call stops the run and shows you the exact
  input first, with Approve once, Always allow this session, and Deny. Takes
  effect on the next mission; Auto never asks either way.

## 0.63.0 - 2026-09-10

- **A Claude Code teammate asks before using a connector nothing has
  pre-approved.** The same approval card as Codex — *Use get_watchlists on
  Robinhood*, what is sent, whether it can be undone — with Approve once,
  Always allow this session, and Deny. "Always" lasts for that connector until
  the mission ends. A denial reaches the teammate with your reason.
- Your own connectors still never ask, as before. This is for the call that
  no rule covered.
- The approval card names the runtime that is asking instead of assuming
  Codex.

## 0.62.2 - 2026-09-10

- **The message-box row is one family of controls.** The `+` and the swarm
  mark are boxed like their neighbours, and the context ring moved to the
  mission line in the header, beside the cost it belongs with.
- **Lime means one thing again.** A selected teammate's card and its open
  conversation sit on the neutral selected ground; lime is kept for what is
  happening right now. In the sidebar, only the state word — *thinking*,
  *done* — takes the colour; the role stays quiet.
- The header no longer says `Starting…` while the thread says it too.
- The Rooms section folds and counts like Teammates and Routines, and its
  door just says `New room` instead of a sentence the rail cut mid-word.

## 0.62.1 - 2026-09-10

- **Replies stream smoothly.** Text used to arrive in jolts and, worse, change
  shape after you had read it - a sentence turning bold, a paragraph becoming
  a code block - because every token re-rendered and re-parsed the whole
  reply. Now tokens land once per frame, and formatting is applied only to
  what has finished arriving; the part still being written is plain until it
  settles, the way Claude Code does it. Measured: zero changes to already-shown
  text across a whole streamed reply.
- The plan for what comes next is written down in `docs/PLAN-2026-09-10-NEXT.md`.

## 0.62.0 - 2026-09-10

- **Ask several teammates at once, from the message box.** Tick a second name
  and the post fans out; the room is where the answers land. Nobody has to
  know rooms exist to make their first one. The box says what sending will do
  before you press it.
- **A room made that way starts as "Untitled room" and is renamed from the
  room** — click its title. Naming it up front was most of why nobody made
  one.
- **Replies are easier to read.** A teammate's reply now runs about 73
  characters a line instead of 111, with more space between lines. Past
  about 85 the eye loses its place on the way back, and it felt "clunky"
  without anything looking wrong.
- **Replies are set in IBM Plex Sans.** Only the reply — every label, chip
  and control stays as it was. The teammate's prose was borrowing the app's
  own typeface, which is built for 13px chips, not nine-line paragraphs.
  Bundled with the app; nothing is fetched.
- The scratch project the testing drives use no longer tells teammates to
  keep answers to one paragraph.

## 0.61.1 - 2026-09-10

- **The Automations screen is now Routines, and it shows you how to make one.**
  It used to describe a right-click. It now lists your most recent finished
  conversations with a Save on each — the actual thing a routine is made from.
- **Save as routine is on the mission header**, beside Activity, on a finished
  conversation. It was only ever in a right-click menu, which is where you look
  once you know an action exists rather than how you find out.
- **What you set up inside a CLI moved to Settings, under that runtime.** Your
  agents, commands and MCP servers are facts about Codex or Claude Code, and
  under them they need no apology — sitting on a screen that otherwise means
  "things you can run" made them read as broken.
- **Ctrl+V a file or a screenshot into the message.** A pasted screenshot has
  no file behind it, so Locust writes it into the same attachments folder the
  + button uses.
- **A teammate's memories stay visible after you reply.** Each reply is its own
  run, and the card was only showing what the latest one learned.
- **A refused command no longer prints itself into the warning.** One line, not
  a whole program; the full command is in the activity row where it belongs.
- **Cursor file rows lose their long paths again.** A Windows path was being
  flattened with one separator left in, so Locust stopped recognising its own
  mirror folder.

## 0.61.0 - 2026-09-10

- **Your teammates can now use your connectors, in every mode.** Whatever your
  Claude Code can reach, they can reach — no list to set up. Driven and
  verified in Accept edits: the row reads `get_watchlists · Robinhood · done`
  and the teammate answers with the number.
- **Worth knowing, and the permission chip now says it:** a connector is not
  on this machine. "Ask · every write is refused" is a promise about *files*.
  A teammate in Ask can still send mail or place an order through a connector,
  because no sandbox here reaches the far end of one.
- **A refused tool is named the way you would say it.** It read
  `mcp__claude_ai_Robinhood__get_accounts`; it now reads `get_accounts on
  Robinhood`, and the reason it gives is one you can act on rather than a
  sentence about running commands.
- A connector you sign into after Locust is open reaches the next mission
  without restarting anything.

## 0.60.1 - 2026-09-09

- **Correction to what 0.60.0 told you about connectors.** It said they were
  off in every mode but Auto because those modes ignore your Claude settings.
  That reason was wrong — those modes never blocked your MCP servers — and
  Locust was blocking them itself. That block is gone.
- **What is actually true, measured:** Claude Code asks before using a
  connector, and a mission has no way to put that question to you outside
  Auto, so the call is refused there. The difference is that you now see the
  refusal — `get_watchlists · Robinhood · failed` — instead of the teammate
  improvising "I have no connection to that", which read as a broken
  connector. The permission chip says which mode can and which cannot.
- Per-connector permission, so a teammate can use one connector without being
  given the whole machine, is next.

## 0.60.0 - 2026-09-09

- **A teammate can work in its own folder.** Open a teammate, and under Own
  branch there is now a "Works in" row. Its missions run there instead of the
  project folder, and nothing else moves: the app does not reopen, your
  history and what the team remembers stay with the project. This is also how
  a teammate reaches an MCP server you registered to one folder — Claude Code
  keeps those per project, so a connector set up in `C:\Users\you\claude`
  exists in that folder and nowhere else.
- **The mode now tells you when it has taken your connectors away.** Only Auto
  runs a Claude Code teammate with your own settings, so only Auto can reach
  an MCP server. Every other mode now says so on the permission chip instead
  of leaving the teammate to answer "I have no connection to that", which
  reads as the connector being broken.
- **Changing the project folder no longer closes Locust.** It reopens there,
  as it always meant to. A runtime whose process ignored the shutdown could
  leave the app with no window and nothing to come back to; the quit now has
  a time limit and leaves anyway.
- **A teammate no longer reports a message to itself as a missing teammate.**
  "Jimothy addressed a message to Finance Bro, who is not on the roster" —
  where Finance Bro was Jimothy's own role. A teammate can now be addressed by
  their role as well as their name, and a message addressed home says that.
- **The teammate dialog said "Ask · every write refused" while the composer
  was in Auto.** It now names the mode the next mission will really run in.
- **The Automations screen has its header back.** Its title had been sitting
  flush against the window edge, with none of the bar every other screen has.

## 0.59.0 - 2026-09-09

- **Your connectors now reach your teammates.** In Auto mode a Claude Code
  teammate can use the MCP tools you have connected — Locust was blocking all
  of them, so a teammate would say the connector was not available and there
  was no way to tell that we had refused it rather than that it was broken.
  The other modes cannot reach connectors at all, because they deliberately
  ignore your Claude settings.
- **A connector call reads as one.** It appeared as a single machine name like
  `mcp__claude_ai_Robinhood__get_watchlists`; it now shows the tool and the
  connector it belongs to, the same way every other tool row does.

## 0.58.0 - 2026-09-09

- **A room post now has one line at the top telling you where it stands.**
  "8 asked · all answered", or "8 asked · 6 answered · Otto running · Sable
  failed" — names when one or two are in a state, a number when more are.
- **And the members as a row of faces beside it.** Click one to jump straight
  to that teammate's answer instead of scrolling for their name. Anyone still
  waiting for a slot appears there too, dimmed.
- **A long answer folds by how tall it is, not by how many line breaks it
  has.** A three-paragraph reply has two line breaks, so it was never folding
  — it just looked right against answers that were one line per number. The
  control now reads "Show the rest".

## 0.57.1 - 2026-09-09

- **Shift+Tab cycles the permission mode.** The mode is the difference between
  a teammate that only explains and one that edits your files, and reaching it
  meant opening a menu every time. It steps only through the modes the
  selected route can actually run.

## 0.57.0 - 2026-09-09

- **The up arrow brings back what you last sent.** Press it on an empty
  message box to get your previous message, again to go further back, and
  down to return to the empty box. Handy for sending the same thing with one
  word changed.
- **Escape stops a running mission.** From the message box, when no menu is
  open.

## 0.56.2 - 2026-09-09

- **Commands your teammate ran now show as commands.** Only one of the
  runtimes was recognised as running a shell, so on Claude Code and OpenCode
  every command appeared as a plain tool row — no result badge, nothing to
  open, and left out of the "commands" count in the turn summary.
- **Correction to 0.56.0.** That release said command rows read as what the
  teammate was doing "where the runtime reports it — Claude Code and OpenCode
  do". The row it appears on did not exist on either of those runtimes, so
  nothing changed on screen; it should work on Claude Code now. On OpenCode it
  is still unconfirmed — we have not seen it send that description at all.

## 0.56.1 - 2026-09-09

- **A room shows its work as it starts.** Posting to a room started everyone
  in turn and only then drew any of it, so you watched an empty room while
  teammates were already working — up to 45 seconds of it. Cards now appear as
  each one begins, with the rest shown waiting for a slot underneath.
- **A teammate in a room is now told its own name.** The briefing listed
  everyone else in the room and never named the recipient, so a teammate asked
  to write to its own file could not tell which name was its own — and wrote
  nothing.

## 0.56.0 - 2026-09-09

- **A room now asks everyone, even when it cannot run them all at once.** Only
  eight missions run at a time, so posting to a full room while other work was
  going left some members simply never asked — no answer, no record, nothing
  the next day to show they had been included. They now wait in line, shown as
  "Waiting for a slot", and start on their own as slots free.
- **"Everyone has answered" waits for them.** It used to be true of whoever
  happened to start.
- **A long answer no longer takes the whole room.** Twelve lines, then "28 more
  lines" to open the rest. One teammate writing at length was pushing every
  other answer off the screen.
- **Past six answers, a room lays them out one per line.** In a grid every row
  is as tall as its longest answer, so two short replies beside a long one were
  paid for in blank space.
- **Command rows say what the teammate was doing, not just what it ran.** Where
  the runtime reports it — Claude Code and OpenCode do — the row reads "Checked
  what the app says about the free route" rather than a shell pipeline. The
  command is still there, one press away.
- **Ticking a ninth teammate into a room says so.** The picker offered everyone
  and then refused the room when you pressed Create room.

## 0.55.3 - 2026-09-09

- **Right-click menus work again.** Pressing any item in a right-click menu
  closed the menu before the press registered, so nothing in it ever ran —
  Delete, Assign to, Copy mission id and Save as routine were all dead. The
  Delete button at the top of a conversation was unaffected, which is what
  made this look like a delete problem rather than a menu one.

## 0.55.2 - 2026-09-09

- **Menus close when you click anywhere else.** The permission mode menu, the
  effort panel and the model picker all stayed open until you pressed the
  button that opened them a second time. Clicking away now closes them, and
  Escape still does too.

## 0.55.1 - 2026-09-09

- **The "Get it" links now open.** Cursor, Antigravity and Node.js each offered
  a link to where you get them, and none of them did anything — in every build
  that had them. They looked like links, and clicking one was silent. Found by
  the first person outside this machine to install Locust.
- **A greyed-out Install button now says why on the screen.** With no Node.js
  installed, four of the five runtimes cannot be installed at all, and the
  sentence explaining that disappeared the moment any one runtime connected —
  which Codex often does by itself. What was left was a panel of buttons that
  did nothing, with the reason only visible if you hovered over one.

## 0.55.0 - 2026-09-09

- **Eight missions can run at once, not four.** The old limit of four was a
  guess: it was written as "a resource bound" and never measured. It has now
  been measured — eight teammates answering at once, each producing a long
  steady stream of output, finished in under twice the time one takes and used
  4.2 GB of memory, with nothing dropped and no run cut short. Nothing the old
  number was protecting against happened. What has not been measured, and may
  change this again, is a machine with much less memory than the one it was
  measured on.
- **A room can no longer be built that it cannot answer.** A room holds up to
  eight teammates, so with the new limit every member of a full room runs. This
  was the cause of the next three fixes.
- **A room no longer says "Everyone has answered" when some of them never
  started.** Posting to six teammates while only four could run started four,
  drew the other two as empty cards, and then told you everyone had answered.
  It now says "4 of 6 in Standup answered; 2 never started."
- **Someone the post could not reach gets a line, not an empty card.** They
  used to get an answer card with no answer in it — a name, "did not start",
  and a blank space where the reply belongs — and the reason was said in the
  smallest text on the screen, far below, repeated once per person. It is now
  one line under the answers: "Pike, Dell and Ember were not asked — up to 8
  missions can run at once."
- **And that reason no longer disappears.** It lived only in the moment of
  posting, so waiting for the room to finish, or reopening it later, left the
  absence unexplained. It is now kept with the post.
- **Ticking a ninth teammate into a room now says so instead of failing.** The
  picker offered every teammate and then refused the room when you pressed
  Create room. It now says "A room holds 8 teammates. Untick 1 to make this
  one." and does not offer the button until it would work.
- **Long answers in a room stop squashing short ones.** Past six answers the
  room lays them out one per line, so a teammate who writes twelve lines no
  longer forces everyone beside them to be twelve lines tall.
- **Warnings and errors that were meant to be amber or red were grey.** Text
  asked to be coloured lost to whatever the surrounding component had already
  set, so several notices — including a room form's own error messages — had
  been rendering as ordinary grey text.

## 0.54.1 - 2026-09-09

- **Two buttons on every teammate card were drawn on top of each other.** In a
  teammate's Routines list, "Run" and "Edit" occupied the same box and read as
  one unusable control, and the line under them was squeezed until the routine's
  next run time was cut off. The name and schedule now have their own line and
  the three buttons sit under them.
- **Locust now finds a runtime you installed into a moved npm folder.** If npm
  has been pointed somewhere other than its default — common on work machines,
  with any Node version manager, and the exact fix Locust itself suggests when
  an install fails on permissions — then the install worked and Locust reported
  the runtime as missing anyway, sending you to install something you had just
  installed.
- **The model list no longer says a model does not exist while it is still
  loading.** Opening the runtime picker and typing in the first seconds after
  launch answered "Nothing matches that", which is a claim about your search
  rather than about the list. It now says it is still reading the list.

## 0.54.0 - 2026-09-09

- **When Codex asks you a question, your answer now reaches it.** In
  "Approve each action", a question from the runtime was drawn with the same
  Approve / Always / Deny buttons as a command — and all three were the wrong
  kind of reply. Codex could not read any of them as an answer, so it recorded
  an empty one and told the model you had said nothing, whichever button you
  pressed. Questions now show what was actually asked, with the options the
  runtime offered, a box for anything else where it allows one, and a hidden
  field where the answer is sensitive. "Always allow this session" is gone from
  questions: there is no such thing for a question, so the button could never
  have meant what it said.
  This is built to Codex's published description of the exchange and tested
  against it, but **it has not yet been seen working against a live question.**
- **A plan is now the answer it is, rather than a card about a run that never
  happened.** In Plan mode the steps are the whole reply, and they were drawn
  in a bordered box with a PLAN label, a "0 of 5 done" counter and one grey dot
  per step — which read as a run stalled at step one. They are now numbered and
  set at reading size, where your teammate's replies appear, with one line
  saying that Plan mode changed nothing and how to have it done for real. The
  plan shown inside a finished run's activity is unchanged, because there the
  steps really do have outcomes.
- **Locust no longer warns that a healthy ledger is damaged.** 0.53.0 added a
  warning for records that could not be read; it counted any older mission that
  had scrolled out of the recent list as unreadable, so a perfectly good ledger
  could report "20 local · 1 file could not be read". A warning is only worth
  having if it is rare enough to believe.
- **And that warning now stays right after you delete missions.** Three places
  re-read your history and only one of them updated the damage state, so
  clearing out old missions could leave a warning on screen that no longer
  matched anything.
- **A command that printed nothing says so.** The case was added in 0.53.0 and
  could not actually be reached: "the runtime reported no output" and "the
  runtime reports output but there was none" had been collapsed into the same
  thing.
- **A failed command's last lines are easier to find.** When a command exits
  non-zero, the end of its output — where the error almost always is — is drawn
  brighter than the rest.
- **The one-line summary of a turn names the command it ran**, so `ran seq 1
  300` is on screen without opening anything, instead of "1 tool call".
- **Several borders were heavier than intended** — the attachment tile, the
  "copied in" mark, the file chip on a sent message and plain notices — because
  they named a colour that does not exist and quietly fell back to the text
  colour.

## 0.53.0 - 2026-09-08

- **Pressing `/` then down then enter no longer switches you into Auto.** The
  command menu listed `/auto` second, so the two most ordinary keystrokes after
  opening it landed on the one mode that lets a run change anything on this
  machine. It is now last, in its own marked section, and arrow-down lands on
  `/edit`. The menu is also attached to the message box instead of floating
  above it, because it is completing what you are typing.
- **A command's output no longer shouts over what your teammate said.** A
  300-line output used to draw a tall scrolling box, inside the scrolling
  conversation, above the two lines the teammate actually wrote. It now shows
  the first eight lines and the last eight, with a button between them that
  prints the rest, and one that copies all of it. The exit code moved to the
  front of the command, because it is what tells you whether the output is
  worth reading. A command that printed nothing says so on one line instead of
  opening an empty box. And the fold's summary now names the command, so
  `ran seq 1 300` is on screen without opening anything.
- **The app no longer says a ledger is verified when it could not read it.**
  Three separate ways it did. A mission whose record was damaged part-way
  through was reported correctly; a record damaged in its header, or too large
  to open, produced no mission at all, so nothing carried the damage and the
  Missions screen said "ledger verified". Worse: if the ledger folder itself
  could not be opened, the screen still said "verified" about a ledger it had
  never read. It now says how many files could not be read, or that the ledger
  could not be read at all.
- **And the card shown when a record cannot be written no longer contradicts
  itself.** It said the mission could not be created, then offered to reopen
  it. Nothing had been written. Because the record is always written before a
  runtime starts, that failure means nothing ran at all — which the card now
  says, rather than warning you to go and check work that never happened.
- **Attaching a file from outside the folder shows one thing, not three.** The
  full-width notice above the message box is gone; the file's own tile now
  carries "copied in", with the whole sentence on hover.
- **A teammate set to "approve each action" no longer runs silently without
  approvals.** Routines, room posts and relayed messages could not show the
  approval cards, so they quietly ran read-only while the composer still said
  approvals were on. All three now refuse and say why, before anything starts.
- **"Up to 4 missions at once" is now actually 4.** Three separate limits of
  four were counted separately, so twelve could run while every count on screen
  said otherwise.
- **A denied action says it was denied.** In per-action approvals, a request
  Locust did not recognise — or one arriving while sixteen approvals were
  already waiting — was refused with nothing said, so the run carried on as if
  you had pressed Deny.

## 0.52.0 - 2026-09-08

- **You can see what a command printed.** A teammate would run something, tell
  you it had printed 1,200 lines, and the row showed the command, the word
  "done", and nothing else - the output had been captured and then thrown away
  before it reached the screen. Command rows open now, showing the start and
  the end of the output with a note saying how much of the middle was left out.
  The end matters as much as the start: an error is usually the last line.
- Rows still stay closed where the runtime reported no output, rather than
  offering to open onto nothing.

## 0.51.1 - 2026-09-08

- **The same receipt failure on Antigravity missions.** Fixing this in "Approve
  each action" prompted a sweep for the same shape, and the Antigravity path
  had it too - a failed write to the durable record was treated as a passing
  read error, so the run kept going unrecorded. It was worse in one way: the
  place it had reached in the agent's transcript moved forward before the write
  succeeded, so those events were skipped for good rather than retried. Both
  fixed.

## 0.51.0 - 2026-09-08

- **"Approve each action" could lose its record of what happened and keep
  going.** If Locust could not write a mission receipt to disk in that mode,
  the failure was discarded: nothing was written, nothing appeared on screen,
  no error was shown, and the run carried on doing work nobody could later
  prove happened. That is the exact failure the durable record exists to
  prevent, and it was in the one mode built for careful, auditable control.
  The run now stops and says so, the same way every other mode already did.

## 0.50.2 - 2026-09-08

- **A rate-limit warning now says which limit, and when it lifts.** It read
  "primary limit 93% used" - the provider's own internal word - when what it
  meant was the five-hour window, which refills within the hour. The weekly
  budget was barely touched. It now reads "5-hour limit 93% used, resets
  14:39", because waiting an hour and stopping for the week are different
  decisions and a percentage cannot tell them apart.
- **And a limit could go unmentioned entirely.** The reader understood only one
  of the two spellings the runtime uses for these numbers, so on the other it
  said nothing at all.

## 0.50.1 - 2026-09-08

- **Picking a teammate now keeps the effort they were set to.** It restored
  their runtime, model and mode and quietly reset the reasoning effort to the
  model default - so a teammate you had set to a low, cheap effort ran at
  medium every time you selected them, and cost more than you asked for.
- **"effort · fixed" is no longer shown before the model list has loaded.** It
  means "this runtime chooses its own effort", and it was also what an empty
  list looked like while the catalog was still arriving - so the composer
  briefly stated something about the runtime that it had no information about.

## 0.50.0 - 2026-09-08

- **When an Antigravity teammate asks you something, you can now read the
  question.** It used to show as "Prompting user with options" and a spinner
  while the run sat blocked - the question and its options were in the record
  the whole time and nothing displayed them. The row now shows what is being
  asked, every option, and that the answer has to be given in Antigravity
  itself. Locust genuinely cannot answer it: Antigravity resolves that question
  by completing the tool inside its own window, and the only channel Locust has
  arrives as a message that starts a new turn instead.

## 0.49.1 - 2026-09-08

- **A routine step that is too long to send now says so while you type it.** A
  step could be saved at up to 20,000 characters when the message carrying it
  holds 12,000 - the step, what the teammate is told about the workspace, and
  anything waiting from other teammates, together. It said nothing until the
  step ran and failed. The warning gives you the number to cut, and saving
  still works: nothing you already have becomes unreadable.

## 0.49.0 - 2026-09-08

- **A multi-step routine no longer disappears when Locust closes.** Quitting
  during step 2 meant the remaining steps simply never ran, with nothing on
  screen to say so - and because the run was counted the moment step 1
  *started*, a half-finished hourly routine then waited out its whole interval
  before trying again. Progress is now written down as it goes, and a run only
  counts when its last step genuinely finished.
- **After an interruption, the routine waits for you rather than guessing.**
  Its card says which step it stopped on, when, and whether that step actually
  completed - and if Locust closed before it could tell, it says that too and
  replays nothing. You choose to continue or abandon. Nothing restarts by
  itself, because a step that has already sent an email or opened a PR should
  not quietly do it twice.
- **Work you have already read stays on screen.** Sending a follow-up used to
  fold away the previous turn's thinking and tool calls, closing something you
  were looking at. Finished turns keep their work now; closing one yourself
  still sticks.
- Every routine card said its step count twice.

## 0.48.0 - 2026-09-08

- **An attached image now looks like the image.** Attaching a screenshot showed
  you the word `screenshot.png` and nothing else, so telling two screenshots
  apart meant leaving the app. Images are drawn as themselves - in the message
  box before you send, and on the message afterwards - beside the name, never
  instead of it. Everything else still shows as a file, and an image that
  cannot be drawn quietly stays a file row rather than leaving a broken picture
  behind.
- Teammates could already read the images you point them at - Claude Code,
  Cursor and Copilot were each measured opening a screenshot and describing it.
  This is the half you could not see.

## 0.47.0 - 2026-09-08

- **You can attach a file from anywhere on your machine now.** Picking one
  outside your project folder used to be refused outright - which was true of
  most runtimes but not a good answer. Locust copies it into
  `.locust/attachments` inside your folder so any teammate can read it, tells
  you it did, and keeps that folder out of git.
- **Attached files sit above the message box, not on the button row.** They
  were a chip beside the mode and folder controls, on a row that does not wrap
  - so attaching something pushed the effort chip off the edge and into the
  swarm mark. Each file is its own tile now, and each one can be removed on its
  own instead of all or nothing.
- **Your own message no longer opens with instructions you did not write.**
  Sending with a file attached put "Read this file in the workspace before you
  answer:" at the top of your own bubble, and used it as the mission's name in
  the sidebar and in search. Your words are the message; the files are shown
  underneath it, and clicking one opens where it lives.

## 0.46.0 - 2026-09-08

- **Type `/` in the message box to reach the controls without the mouse.** A
  menu opens listing what you can do from here - `/edit`, `/auto`, `/model`,
  `/swarm`, `/stop` - with a sentence under each saying what it does. Arrows
  pick, Enter runs it. Only what is genuinely available is listed: `/stop`
  appears while something is running, and a mode the chosen model cannot honour
  is not offered at all, so nothing in the menu can be chosen and then refused.
  A slash inside a sentence is left alone - "run /plan on this file" is still a
  message, not a command.
- **Auto's warning is finally the colour it was meant to be.** The line saying
  Auto "may change anything on this machine" was written to stand out in amber
  and had been rendering in the same grey as everything else since it was
  added.

## 0.45.4 - 2026-09-08

- **A teammate writing a lot no longer gets stopped for it.** A long report on
  Cursor Agent died with "sent more output than Locust could take in" partway
  through - not because anything was wrong, but because Locust would only hold
  64 lines of output while it wrote the previous batch to disk, and a fast
  model writes more than that in the time one save takes. It holds far more
  now.

## 0.45.3 - 2026-09-08

- **A routine that asks you something now waits for the answer.** A step that
  ended by asking a question counted as finished, so the next step started
  immediately - and your answer was then refused because the teammate was
  already busy with it. The routine stops at that step and says so.
- **A busy workspace no longer looks like a broken one.** When four missions
  were already running, a scheduled routine treated the refusal like a
  signed-out CLI and waited an hour for a slot that often frees in a minute.
  It now waits for the next minute, like it does when the teammate is busy.
- **A long routine step no longer throws away waiting messages for nothing.**
  A step long enough to exceed what can be sent would drop every message
  waiting for that teammate trying to make room, then send anyway - losing the
  messages and fixing nothing.

## 0.45.2 - 2026-09-08

- **What a teammate did stays on screen after it finishes.** While a run is
  going you watch it think and call tools; the moment it ended, all of that
  collapsed into a single line. The newest finished turn now keeps its work
  open - the tools it called, the files it changed, the diffs - so you can read
  it afterwards, or catch what you missed. Earlier turns stay collapsed, and if
  you close one by hand it stays closed.

## 0.45.1 - 2026-09-08

- **Copilot CLI's effort levels work.** It accepts seven of them - none,
  minimal, low, medium, high, xhigh, max - and Locust both failed to read them
  and refused to send one, so the composer said the effort was fixed when it
  was not. Locust now reads the levels a runtime names in its own help,
  whichever way it writes them, and passes your choice through.
- **The Fast variant is a switch**, not a button that fills with colour.

## 0.45.0 - 2026-09-08

- **Attachments.** The `+` is back on the composer and it does something: it
  opens a picker limited to the folder your teammates work in, and the files
  you choose are named at the top of your message so the teammate reads them
  before answering. It works on every runtime, because reading a file in the
  workspace is the one thing all six can already do - and the read shows up in
  the activity fold, so you can see it happened rather than take Locust's word
  for it. Files outside the folder are refused, and a file's contents are
  never pasted into the message.
  The chip says "2 files" rather than "attached", because on most runtimes
  nothing is attached in the technical sense. Three of the six take a file
  natively and a later build will use that, and say so when it does.

## 0.44.1 - 2026-09-08

- **Your conversations are reachable from the narrow layout.** In the 64px
  avatar rail, hovering a teammate opens their conversations beside it - name,
  role, what they are doing, and each conversation with how many turns and how
  long ago. Click the avatar to pin it open; Esc or a click elsewhere closes
  it. A small count on the avatar says how many conversations are behind it.
  Right-click a conversation there for the same menu as the full sidebar,
  including Save as routine.

## 0.44.0 - 2026-09-08

- **Teammates stop losing what they said.** A long reply had its ending cut
  off, and everything a teammate uses to reach you or another teammate - a
  message to a colleague, something learned worth remembering, a question, a
  room task - is written at the end. So the longest, most substantial turns
  looked perfect in the thread and quietly delivered nothing. Long replies now
  keep both ends, and on Cursor Agent a long answer is properly closed instead
  of being left half-finished.
- **Three instructions that contradicted each other.** Memory, questions and
  room tasks each told a teammate its block had to be the last thing in the
  reply. A turn that needed two of them had to disobey one, and dropped it.
  They now say what was always true: put them at the end, in any order.
- **Sending to a teammate is harder to get wrong.** Writing `to='Gem'` with
  single quotes silently sent nothing, and copying a teammate's name from the
  roster exactly as shown - `Gem (Custom)` - was refused as not on the roster.
  Both work now.
- **A numbered list of choices is a question again.** If a teammate offered
  "1. Rewrite it  2. Patch it", no card appeared and the run simply ended.
- **A leftover effort can no longer fail a run.** Switching to a runtime that
  takes no effort level - OpenCode, say - while one was still set from the
  previous model failed the run outright: "OpenCode takes no effort level.
  Nothing was recorded." The effort chip also stayed on screen showing a level
  with nothing behind it, so there was no way to clear it. Locust no longer
  sends an effort to a model that has none, and no longer offers one.
- **A stalled Antigravity run says what it is stuck on** instead of reporting
  that the agent wrote nothing. If it is waiting on its own question, Locust
  now says so and tells you to answer it in Antigravity's window.

## 0.43.9 - 2026-09-08

- **Effort is a slider now, not a list.** Cursor Agent offers eight levels and
  the old menu listed all eight with a sentence under each, which ran off the
  bottom of the window and off its right edge. Those eight are really four,
  each with a faster variant - so it is a Faster-to-Smarter slider with a
  "Fast variant" switch, using whatever levels the chosen model actually
  offers.
- **A runtime's models turn up without a restart.** Locust calls a runtime
  ready as soon as it is installed and signed in, which happens before it has
  finished asking that runtime what models it has - and nothing asked again.
  If you caught it at the wrong moment, a runtime offered only
  "account-default" until you restarted the app.
- **Assigning a conversation says so.** It moved a row in a list you may not
  have been looking at, so a successful assign and a failed one looked
  identical. It now confirms which teammate it went to.

## 0.43.8 - 2026-09-08

- **You can choose the sidebar layout.** Settings now offers Auto, Full and
  Rail. Auto still follows the window - the compact avatar rail on a small
  window, the full sidebar on a large one - but you can pin either, at any
  size. The rail also stopped drawing conversation rows squeezed into four
  pixels, which is what was causing the stray dots, the sideways scrollbar and
  the tall empty box around the selected teammate.
- **An approval no longer yanks the screen.** The approval card used to scroll
  itself into view whenever it appeared, which was right the first time and
  wrong every time after - a second approval, or one arriving while you were
  scrolled up reading an earlier diff, took the page away mid-sentence. The
  thread now decides, and it only follows if you were already at the bottom.
- **The effort chip stopped disappearing on Cursor Agent.** After a run
  resolved the route to a specific model, Locust stopped recognising its own
  model and the chip vanished. A runtime that does not let you choose an
  effort now says so, rather than leaving a gap.
- **Amber means something again.** It marks a card that is waiting on you to
  press something. The stopped-run summary and the "cannot be resumed" note
  were wearing it while asking for nothing, so they now sit quietly with the
  other notes.
- **A stopped run lists what it finished properly.** It was showing the same
  file twice - once with its full path, once without - and the folder itself
  as a long absolute path above three short filenames.

## 0.43.7 - 2026-09-08

- **Locust works properly in a small window.** The layout meant to appear on a
  laptop-sized window had been unreachable for days: it was written for windows
  narrower than the smallest one Locust will open. Now, at the minimum size,
  the sidebar becomes an avatar rail and the inspector slides over the
  conversation instead of vanishing - it used to disappear entirely at that
  width, taking the signal rail, the receipt and the artifacts list with it.
- **Hovering a teammate says who they are.** Name, role and the model they run
  on. In the narrow layout that hover is the only way to tell one avatar from
  another.
- **The effort menu says what each level costs you.** Fast, medium, high and
  the rest each explain themselves in terms of speed and money rather than a
  promise about the answer. On Cursor Agent it also says that the effort is
  part of the model name, which is why choosing one changes the model shown.
- **A ledger Locust cannot write is now a proper screen.** It says what is
  safe, what is at risk, and that stopping the run did not undo files it had
  already changed - and it will open the folder for you. It used to say the
  mission was "held" and, in the next sentence, that it had been stopped.

## 0.43.6 — 2026-09-07

- **A long run is no longer killed by one big piece of output.** A mission on
  Cursor Agent was stopped a minute and a half in, after 1,596 pieces of a
  perfectly normal answer, because the 1,597th was larger than Locust would
  take in one go - and everything already on screen was thrown away with it.
  That one piece is now skipped and the run carries on. If a run later ends
  with nothing to close it, Locust says a piece was skipped rather than
  blaming the runtime for a silence Locust caused.
- **Cursor Agent works again with an effort chosen.** After one turn, a
  follow-up on the same conversation failed at once with "Cursor Agent takes
  no effort level. Nothing was recorded." Cursor carries the effort inside the
  model name, and Locust had stopped recognising its own model once that name
  included one.
- **You can open the file your teammate just wrote.** Every changed file in
  the activity fold now has a control that shows it in File Explorer, and the
  inspector's Artifacts tab lists what the mission produced instead of always
  saying there is nothing. Locust shows the file rather than opening it: a
  script a model wrote should not be one click from running. Anything outside
  the folder your teammates work in is refused.
- **OpenCode says why it stopped.** Running out of the free model's usage
  read as "OpenCode ended without a step that reported it had stopped". It now
  says the free model has no usage left and to pick another one or come back
  later.
- **The thread follows the newest line.** It stays at the bottom while you are
  at the bottom, and stops the moment you scroll up to read something - it
  will not yank you back mid-answer. The scrollbar is thin now, with no
  trough.
- **The composer stopped explaining Shift+Enter.** It was taking up room in
  the box to say something everybody knows.
- One written file no longer draws two identical rows in the activity fold.

## 0.43.5 — 2026-09-07

- **An approval now says what would leave your machine.** The card told you
  what would happen, where, and whether it could be undone — never what data
  it sends, which is the one question you cannot work out for yourself. A file
  change says "Nothing. The change is written to this machine and sent
  nowhere", because that is provable. A command that names a network tool says
  it *can* reach the network and that Locust cannot see what it would send.
  Any other command says plainly that this is unknown — Locust will never tell
  you a command sends nothing, because `./deploy.sh` is two words and can do
  anything.
- **A question says you can just answer it.** The decision card offered two
  buttons and looked like it took nothing else. It now says the composer works
  too, so a third answer does not have to be squeezed into the closest wrong
  option.

## 0.43.4 — 2026-09-07

- **A card no longer says "no files changed" when it cannot know.** With two
  teammates working in one folder, the app already says so plainly — "what
  changed on disk cannot be told apart ... it is not counted as this run's
  work" — and then, on a run whose own runtime reported no edit, put "no files
  changed" directly above that sentence. Measured with one teammate on
  OpenCode and one on Cursor at once: the card said nothing had changed while
  the file that run had just written sat on disk beside it. Knowing nothing is
  not the same as knowing nothing happened. A run that genuinely changed
  nothing still says so.

## 0.43.3 — 2026-09-07

- **A routine taught on Cursor replays again.** Saving one recorded the
  reasoning level beside the model, and Cursor keeps its levels *inside* the
  model id and refuses one passed separately — so a routine taught on Cursor
  threw the moment it replayed and never opened a mission. It had been that
  way since 0.43.0. The level is now stored only where it would actually be
  sent, so a routine on Codex or Claude Code still replays at the level it was
  taught with. A routine saved on Cursor by 0.43.0, 0.43.1 or 0.43.2 still
  carries the bad route: save it again and it will run.

## 0.43.2 — 2026-09-07

- **The next thing you type no longer goes missing.** Typing a second line
  before a teammate had finished starting could leave it stuck in NEXT under
  "that conversation is no longer open" — about the conversation on screen —
  where it sat until you noticed and sent it by hand. A mission moves to its
  real id the moment the host answers, and anything queued against it now
  moves with it. The same applies across a handoff, where the conversation
  carries on under another runtime.

## 0.43.1 — 2026-09-07

- **One file changed once is counted once.** A one-line append could report as
  `2 files · +2 −0` when git said one line in one file: some runtimes restate
  the same edit with the other spelling of the path, once relative to the
  folder and once absolute, and both the count and the file list took those
  for two changes. They are folded now, on the path as the list draws it. Two
  genuine edits to one file still count as two.

## 0.43.0 — 2026-09-07

- **A routine replays at the effort it was taught with.** Saving one records
  the level you had set; replaying it uses that instead of falling back to
  whatever the runtime does by default. A route whose model reports no levels
  stores none, so nothing changes on the runtimes that take no effort.
- **A teammate stops volunteering another teammate's work.** Shared memory
  stays on — it is useful, and each memory already names who wrote it — but a
  teammate was bringing one up when nobody had asked, reporting what a
  colleague had done to a file you were not asking about. It is now told when
  *not* to raise a memory, which the brief had never said.

## 0.42.0 — 2026-09-07

- **The effort chip agrees with the model beside it.** While a mission ran, the
  chip named the live model but took its levels from whatever route you had
  queued next — two models on one chip.
- **Every turn says when it changed nothing.** `no files changed` was written
  onto the newest turn only, so scrolling up in a conversation showed the same
  silence the line exists to break.

## 0.41.3 — 2026-09-07

- **A conversation shows that it has a menu.** Saving a routine needed a
  right-click, and nothing said so — which is why the feature looked missing
  even after the Automations screen started naming the gesture. Hover a
  conversation in the sidebar (or tab to it) and the actions are there,
  `Save as routine` among them.

## 0.41.2 — 2026-09-07

- **Resuming a mission keeps the effort you set.** It sent the model and
  nothing else, so a resume quietly fell back to the runtime's own default —
  and on Cursor, where the effort is part of the model id rather than a
  separate flag, it ran a different model than the one on screen.

## 0.41.1 — 2026-09-07

- **Swarm is in Settings.** Its only control was the mark on the composer,
  which exists on the workroom alone and is disabled while a mission runs — so
  a workspace-wide setting could not be reached from any other screen, or
  turned off while anything was running. The mark is the glance; Settings is
  the record.
- **A runtime that is already installed stops offering to install itself.**
  One whose version probe has not answered yet was tagged CHECKING and fell
  through to the Install button — or to `Get it ↗` for Cursor and Antigravity.
- **A connected runtime with no version shows its status instead of nothing.**
  The check was written against `undefined` where the value is `null`, so it
  never fired and the slot came out blank — on Antigravity, whose version can
  genuinely be absent.

## 0.41.0 — 2026-09-07

The two things that stopped a stranger, from the full-scope plan.

- **A fresh install lands on a route that exists.** `account-default` means
  "use whatever your account uses", and every runtime honours it — but only
  Codex had a row for it, so once a new machine started preferring OpenCode
  you landed on a lowercase placeholder with no effort control and nothing
  marked as your current route. Every runtime has its own **Account default**
  row now.
- **The install screen shows its work.** The command is shown while it runs
  rather than only after it fails, and `Show output` opens onto everything npm
  said — on the live line and on the failure, where the last lines are usually
  the cause.

## 0.40.1 — 2026-09-07

Three more from the outside tester's report.

- **The two mission counts explain each other.** The sidebar is scoped to the
  folder you are in; the Missions screen is the whole ledger. Both were right
  and neither said so, so one read 3 while the other read 5. The header now
  says `5 local, 0 in this folder`, and only adds that clause when the two
  actually differ.
- **The model chip stops naming the runtime twice.** `OpenCode /
  opencode/ling-3.0-flash-fin-free` truncated to `OpenCode / opencode/big-pic…`
  — the runtime twice and the model cut off. A provider that merely repeats
  the runtime is dropped; one that is real information (`anthropic/…` under
  OpenCode) stays.
- **OpenCode names the output cap.** It was the last of the five runtimes that
  went quiet when Locust stopped a run for sending more than it accepts.

## 0.40.0 — 2026-09-07

- **Automations shows what you set up in the CLIs.** Agents, commands and
  automations you configured in Claude Code, Codex or Cursor are listed in one
  place — with what each one is for, and where to find the file. Locust did
  not make them and does not run them, so they sit apart from your routines
  and have no buttons. A machine with nine of them on it used to read
  "Nothing saved yet."
- **The Automations screen stops sending you to the wrong place.** It said to
  save a routine "from that teammate's card", under a button reading "Open the
  team" — and the control is on neither. It is a right-click on a conversation
  in the sidebar, which is what the screen now says.

## 0.39.2 — 2026-09-07

- **An npm permission error now shows the fix.** It used to say "run the
  command below in a terminal with permission to install global packages" and
  show the same `npm install -g` that had just failed — which fails the same
  way, because the unwritable thing is npm's global folder, not the terminal.
  It now gives `npm config set prefix "<a folder you own>"` and the install,
  and says to put that folder on your PATH.
- **A machine with nothing installed no longer claims Codex.** The welcome
  screen recommends OpenCode while the composer said `Codex CLI /
  account-default`. They agree now.
- **The Memory screen's notice can be dismissed.** Once any teammate
  remembered, proposed or forgot something, that sentence stayed for the rest
  of the session — including after you acted on it on that very screen. An
  update where nothing actually changed also drew an empty paragraph.

## 0.39.1 — 2026-09-07

- **Every runtime names the output cap, not just Codex.** Locust stops a run
  whose single line of output passes 256 KB. Codex has said so since 0.38.4;
  Claude Code, Cursor Agent and Copilot CLI share the same cap and said
  nothing, so a huge-output run looked like the model shrugging rather than
  Locust stopping it. (OpenCode's terminal path has a different shape and is
  still to do.)

## 0.39.0 — 2026-09-07

From a first outside tester's report and an audit of the six releases before
this one. **It corrects 0.38.7, which claimed something untrue.**

- **The composer says how to type a new line.** Enter sends and always did;
  Shift+Enter makes a new line and nothing said so. A tester typed a two-line
  prompt, the first Enter submitted the first line, and the rest queued behind
  it as NEXT until the conversation had closed — losing their opening mission.
  The hint appears while there is something in the box and nowhere else.
- **The effort on the chip is the effort the run is given.** It was not.
  0.38.7 said *"what the chip says is what the run is given"* and that was
  false: the chip rendered a default that was never assigned, so from every
  launch it read `medium` while the run was started with **no effort argument
  at all**. Display and dispatch now share one expression.
- **Changing the mode no longer promises writes it cannot deliver.** Set to
  Accept edits on a thread begun in Ask, the follow-up still ran with no write
  tools, because a resumed runtime session keeps the tools it was built with.
  A changed mode now starts a fresh session instead of silently inheriting the
  old permissions.
- **The typecheck gate is green again.** `pnpm typecheck` runs two configs and
  I had been running one, so three releases were cut over eight errors — six of
  them dead props left by 0.38.7, two in a test file. The dead props are gone,
  along with two comments that described the opposite of the shipped code.

## 0.38.9 — 2026-09-07

- **A run that was allowed to edit and edited nothing says so.** Cursor Agent
  said "Applying the two edits to notes.ts now", reported completed, and left
  the file untouched — and nothing on screen contradicted it, because the
  file count is only drawn when it is above zero. The fold now ends in
  `no files changed`. Stated plainly rather than in amber: asking a question
  in an edit-permitted session changes nothing either, and that is fine.

## 0.38.8 — 2026-09-07

- **Cursor's file rows name your file again.** Cursor Agent reports what it
  edited from inside its own copy of the project
  (`~/.cursor/projects/C-Users-you-code-streaks/src/streak.js`), and a real
  two-line edit produced eleven rows like that, with the filename pushed off
  the end of the row. That folder name is the workspace path with its
  separators flattened to hyphens, so it is recognised rather than guessed:
  only a mirror of the folder you actually opened is read as your file.
  Another project's mirror keeps its full path.

## 0.38.7 — 2026-09-07

Effort is its own control again, and it always says something.

- **A separate effort dropdown, beside the model.** Folding it onto the route
  chip made it invisible until chosen, and choosing a model cleared it — so
  picking a model instantly left you with no effort and no way to see one.
- **Picking a model no longer empties it.** The level carries across when the
  new model advertises it, and lands on that model's default when it does not.
  Never on nothing. What the chip says is what the run is given.
- **Nothing about effort in the model list any more.** Each row still names
  the levels a model reports, because that is information worth having while
  choosing a model; choosing between them belongs on the composer.

## 0.38.6 — 2026-09-07

- **The swarm mark is back on the composer.** The design review moved it into
  the route picker's header; it belongs where you can see it. It is the app's
  own logo and it says, at a glance, that every mission is running at its
  model's maximum. It is never disabled now either — swarm is a statement
  about every mission, not about the one route you happen to be on. The
  picker keeps the consequence rather than a second switch: the effort levels
  grey out and say who is holding them.
- **The route chip has a chevron.** Effort moved behind that chip, and nothing
  said the chip opened anything.
- **A connected runtime never offers to install itself.** Antigravity showed a
  green dot and a `Get it ↗` button in the same row, because its tag is
  EXPERIMENTAL rather than READY and the row fell through to the download
  branch. A row cannot say connected and not-installed at the same time.

## 0.38.5 — 2026-09-07

Effort on the route you actually start on. 0.38.4 put the control back but
only on routes that name a model; this is the one that fixes a fresh install.

- **The account default is a row you can select.** It is the route a new
  profile starts on, and it was the only route in the app naming a model no
  list contained — so the picker had no ACTIVE row for it, and the effort
  levels, which sit under that row, had nothing to attach to. It now appears
  as **Account default**, carrying the levels every model on your account
  agrees on. An intersection, not a union: a level only some models accept
  would be a control that silently does nothing.
- **The route you are on is the first row in its group.** It sorted by the
  same rules as everything else, so a route that is neither recently used nor
  a famous name sank below six models and behind a "1 more model · type to
  search them" line. The row the composer points at should never need finding.

## 0.38.4 — 2026-09-07

The effort control, which 0.38.1 removed and did not replace.

- **You can choose reasoning effort again.** 0.38.1 dropped the composer's
  `effort · fixed` chip, and the chips that replace it — under the selected
  model in the route picker — landed after that build was cut. So the shipped
  app had the old control gone and the new one absent. Open the route picker
  and the levels sit under the model you are on; the one you pick rides on the
  route chip as `Claude Code / sonnet · high`.
- **Swarm is always there.** It was drawn only when the selected route
  reported effort levels, so on a route that reports none — including the one
  a fresh profile starts on — the setting vanished from the app entirely
  rather than moving. It is a pill in the picker's header now, always.
- **A run stopped for output volume says so.** Locust caps a single line of
  runtime output at 256 KB and kills the process past it. It used to report
  "Codex invocation did not complete successfully" — the words it uses when it
  has no idea what happened — while knowing exactly what happened. It now
  names the cause and suggests narrowing the ask.
- **The finished-exchange line is still the exchange.** Collapsing it dropped
  its identity along with its band, so screen readers lost it.

## 0.38.3 — 2026-09-06

The rest of the design pass, matched to what was actually drawn rather than to
the description of it.

- **The reasoning effort rides on the route chip** — `Codex CLI / gpt-5.6 ·
  high` — instead of sitting in the picker as a row of its own. It is a
  property of the route, so it reads as part of it.

- **Swarm is a pill beside the picker’s search**, not a band across the top of
  the list. It is one setting, not a section.

- **The activity fold counts the plan’s steps** in its own summary line:
  `41s · 3 of 3 steps · asked 1 subagent · 6 tool calls · 3 files`.

- **A teammate’s message to a teammate is drawn as the same bubble** as
  everything else said in the thread, rather than one with its own size,
  padding, corner and border. Who sent it is a label above it.

## 0.38.2 — 2026-09-06

Two things found by pressing the Install button for the first time.

- **An install that worked no longer looks like nothing happened.** The
  package landed, the command was on disk, and the screen still said no
  runtime was connected and still offered to install it. It notices
  immediately now.

- **A machine without Node is told so, before it is offered a button.** Four
  of the five runtimes install through npm, so without it those buttons cannot
  work — and pressing one used to report that npm had stopped with an error,
  which blamed the command for not existing. The panel now says Node is
  missing and links to it instead.

- **A failed install keeps the command on screen**, with a button to copy it,
  so it can be run by hand or passed to someone who can read it.

## 0.38.1 — 2026-09-06

A design pass over the conversation column, which had grown nineteen different
kinds of object where Claude Code has three. Nothing is removed; several
things stop being their own box.

- **A plan now sits inside the activity fold**, as its first rows, instead of
  in a card above it. A plan is the clearest statement of what a run did, so
  it belongs with the rest of what it did. A run that answers with steps and
  touches nothing keeps its plan where it was.

- **What a conversation taught the team** is a note now rather than a bordered
  card. It already happened and it asks nothing of you.

- **The mission id is said once.** It sat at the top of the thread and in the
  header band above it, thirty pixels apart. The start time was only in the
  thread, so that stays.

- **The durable receipt is one line** — runtime, model, checkpoints, verified —
  with the full table behind the same disclosure the activity fold uses.
  Nobody reads "Events: 41 recorded" twice. An unverified action still shows
  without opening anything.

- **A teammate's row in the sidebar stops stacking five lines.** The routine
  step and the branch answer the same question, so one line shows whichever
  applies: the step while a routine runs, the branch otherwise.

## 0.38.0 — 2026-09-06

- **Locust installs a coding agent for you.** Open it on a machine with none,
  and each one now has an Install button beside it instead of a command to copy
  into a terminal you have to find. OpenCode leads the list and is the only one
  with a filled button, because it is the only one that needs no account at all
  — one install and there is a working teammate.

- **It shows the line it is about to run, before it runs it**, and while it
  works it shows npm's own last line and how long it has been going. There is
  no progress bar, because npm does not report anything that honestly becomes
  one.

- **When an install fails it says what happened and what to do** — no network,
  a proxy, permissions, a name that has moved, or something nobody has seen
  before — and the command stays on screen so you, or someone helping you, can
  run it by hand. If npm finishes cleanly and the command still is not there,
  it says that too and offers a restart.

- **Cursor Agent and Antigravity are not packages**, so their button opens
  their own page instead. Same size, same place, one different word.

- **Signing in has no button and will not get one.** It happens in a browser
  or with a device code, and a button that opened a terminal and left you in it
  would be worse than the line telling you what to type. The row shows the
  command, and Locust notices on its own when it is done.

- **The composer lost three controls it did not need**: a `+` that was
  permanently disabled, an effort chip that read "effort · fixed" on most
  routes, and a swarm toggle that switched the effort chip off. Effort now sits
  under the model it belongs to in the route picker, and swarm is that picker's
  own switch.

## 0.37.1 — 2026-09-06

Three corrections, two of them to fixes from earlier the same day.

- **A teammate replying to another is no longer told the folder was shared.**
  When one teammate passes work to another, the second was told "another
  teammate was working in this folder at the same time, so what changed on
  disk cannot be told apart" — about a run that had already finished. Its own
  work then counted for nothing. Two runs share a folder when both are
  running in it, which is now what gets asked.

- **Teammates on their own branch are told so even in a folder with no
  LOCUST.md.** The sentence that stopped those runs dying was riding along
  with the project's own instructions, so a folder without an instructions
  file never got it — which is every folder, for someone who has just
  installed the app. The team's memory also stopped naming the main folder to
  a teammate that is not standing in it.

- **A run stopped by the folder boundary says which folder.** It used to say
  "OpenCode ended without a step that reported it had stopped", which
  describes the silence rather than the cause.

- **A room teammate that repeats the example no longer files it as work.**
  Every teammate in a room is shown an example of how to update the task
  board, and repeating that example back put its placeholder text on the
  board as a real task.

## 0.37.0 — 2026-09-06

- **Install the runtime Locust asked you to install, and it just works.** It
  did not before. The composer was pointed at Codex CLI from the moment the
  app opened and never moved, so someone who installed OpenCode — because
  Settings told them to — came back to a box still reading "Install a coding
  agent and sign in to start a mission", pressed Enter, and got nothing at
  all. The route now follows what is actually on the machine, preferring the
  one that needs no account; a route you pick yourself is never moved for you.

- **A message that cannot be sent says why.** Pressing Enter used to do
  nothing, silently, whenever the route could not run. That is the worst thing
  a first run can do.

- **The model list keeps up.** It was read once when the app opened, so a
  runtime whose check finished a moment later showed a single "account
  default" row for the rest of the session — Claude's Sonnet, Opus and Fable
  simply missing, and every OpenCode model too. It is re-read when the
  runtimes change and when you open the picker.

- **The picker says which models are free.** OpenCode's free ones were listed
  by name with nothing to distinguish them from the paid ones.

- **A runtime that needs no account is no longer described as signed in.**
  OpenCode said "Signed in on this machine, using your own account" to people
  who had never signed in to anything.

- **The first screen and Settings stop asking for a sign-in that is not
  needed**, and on a machine with nothing installed the list leads with the
  one that needs no account instead of burying it fifth.

## 0.36.5 — 2026-09-06

- **A runtime Locust cannot find now tells you how to get it.** It used to say
  only "Claude Code was not found on this machine. Install it and sign in" —
  true, and a dead end that sends you off to search. Each one now shows the
  exact line to run, with a button that copies it, and says what signing in
  takes afterwards. The two that do not install from a package manager link to
  their own page rather than to a command line invented for them.

- **And on a machine with none of them, it says which one to start with.**
  OpenCode needs no account at all — one command and its free model runs —
  so that is the sentence at the top of the list, instead of leaving you to
  read down a list that opens with one wanting a paid subscription.

## 0.36.4 — 2026-09-06

- **Teammates on their own branch actually finish now.** Giving each teammate
  its own worktree is how you stop several of them colliding in one folder, and
  it was the least reliable way to run them: with three going at once, six of
  nine runs died with nothing but "the run could not continue". A teammate was
  being told its instructions belonged to the main folder, which is not the
  folder it works in, so it went looking — and OpenCode ends a run that asks
  for a directory outside its own. A teammate on a branch is now told what is
  true for it: this is the project, you have your own copy, work inside it. Nine
  runs since, none lost, and nothing written outside anyone's own copy.

## 0.36.3 — 2026-09-06

- **A shared folder now says it is shared.** 0.36.2 stopped a teammate taking
  credit for another's files, and went too quiet doing it: a run that had just
  edited a file could show five tool calls and no file at all. It now says what
  it honestly knows — "another teammate was working in this folder at the same
  time, so what changed on disk cannot be told apart" — and names how many
  files in the folder are different, counted against nobody. What each model
  reports about its own work is still counted as its own.

## 0.36.2 — 2026-09-06

- **Teammates working at the same time no longer take credit for each other's
  files.** Start three teammates in one folder and each activity card counted
  every file all three had changed: a teammate that wrote a single forty-line
  file reported three files and a hundred and twenty-four lines. The host was
  comparing the whole folder before and after each run, which is how it catches
  an edit a model makes quietly and never mentions, and it cannot tell whose
  edit it is when two are working at once. Now it says nothing rather than
  something wrong, and what each model reports about its own work is unchanged.

- **A teammate that cannot be read from the roster file says so** in the log
  instead of simply not appearing.

## 0.36.1 — 2026-09-06

- **"Install and restart" restarts.** It never did: installing on quit is
  silent and starts nothing, and the call that relaunches could not be
  made while the app was still flushing its record. The installer now runs
  as the last act of that shutdown, with the flag that starts the app
  again (Colin: "the restart after update and restart has never worked").
- **A plan is still a plan after a restart.** Reopening one lost its
  "Build this plan" offer and told you instead that the change was only in
  the reply, which is the wrong thing to say about a plan. The record now
  keeps which mode was asked for, beside what the run was allowed.

## 0.36.0 — 2026-09-06

A sidebar you can fold, a place for automations, and eight fixes from an
outside review.

- **The sidebar folds.** Teammates and Automations are sections with a
  count and a chevron, and the whole heading is the button. Missions
  appears when there are conversations no teammate owns, and says just
  "Missions". Every conversation is still one click away under its
  teammate.
- **Automations.** Every routine in one place, scheduled ones first,
  with its teammate, steps, runs and next run. The section stays on the
  sidebar even when it is empty, so the capability is visible before you
  have used it.
- **Settings toggles are switches**, the same control the Memory screen
  has always had.
- **How full the model's context is**, as a small ring beside the route.
  Hovering says it in words. It is drawn only where the runtime reports
  its own window size, so nothing is measured against a guess.
- **Real model names.** Claude Code takes an alias — sonnet, opus,
  fable — and only its result says which model that meant. The picker
  now learns from there, so a route reads "Sonnet · claude-sonnet-5"
  after its first run.

Fixed, from a review that ran the app rather than reading it:

- **Memory's "Open the conversation" opened nothing**, every time.
- **A handoff could take its title from your own words** when they
  happened to contain a sentence the app writes into its briefings.
- **The activity fold counted the wrong things.** A model's own to-do
  list counted as a changed file; a deleted file counted as none.
- **A scheduled routine that could not start failed silently** and kept
  failing. It now says which one, why, and when it will try again.
- **A long name no longer wraps the controls row** at any window size.
- **A borrowed route says what it may do.** A teammate replying on
  someone else's route never inherits Auto, and now says so.

## 0.35.2 — 2026-09-06

From a targeted QA pass on 0.35.0, and one thing Colin saw on the bar.

- **The composer addresses whoever owns the conversation you opened.**
  Clicking a teammate's message in an exchange opened their run under a
  header with their name, while the composer still said "Message Wren…"
  and Wren's card stayed lit. Three surfaces, two answers, on the one
  control whose job is saying where the next message goes.
- **"2 files" over one file.** The fold counted rows, so the same file
  edited twice was two files. It counts distinct paths now.
- **A refused write is not a changed file.** It was being counted as one,
  and counted again as a refusal.
- **A handoff shows the words you actually typed.** When a route switch
  carries a new instruction, the thread was reaching past it to the
  sentence that opened the conversation. A rescue with nothing new still
  shows the original.
- **Auto is always in the permission menu.** It used to be absent until
  you switched it on in Settings, so choosing it meant going somewhere
  else first (Colin: "always allow auto to be chosen from the permission
  dropdown, we want the user experience to be fluid"). Picking it is what
  switches it on. Settings still shows the state and takes it back, and
  the app still asks that switch as each run starts.
- **A long name no longer breaks the controls row.** OpenCode's free
  model is `opencode/muse-spark-1.3-contributor-free` and a scratch folder
  is not much shorter; the row wrapped to a second line to fit either one
  (Colin: "so long in txt it collapses below", then "maybe got to auto
  shorten file text as well"). The name gives way instead, and each chip's
  tooltip carries the whole thing.

## 0.35.1 — 2026-09-06

- **A turn that wrote to a teammate is not a silent turn.** "This turn
  ended without a reply" was appearing directly above the message the
  turn had just sent: those messages are drawn beside the thread rather
  than in it, so the check never saw them (Colin: "the this turn ended
  with a reply intended?"). A turn that genuinely wrote nothing still
  says so.
- **The exchange pill says which way the message went.** "1 message to
  Booty" or "1 message from Booty" instead of "with", and the row
  underneath no longer repeats the sender the pill just named.

## 0.35.0 — 2026-09-06

- **Auto mode: a run that is not confined to the workspace folder.** Off
  until you switch it on in Settings, and then offered in the composer
  beside the others, saying what it does in amber: "Runs without asking
  and may change files anywhere on this machine, not only this folder."
  Every other mode still refuses a write outside the folder. The switch
  is checked again each time a run starts, so turning it off stops the
  next one — including one a teammate or a saved routine was about to
  start — and the mission's record says `auto · whole machine` rather
  than leaving a reader to guess what that run was allowed.

  What each runtime is actually given, measured off its own `--help`:
  Claude Code `--permission-mode bypassPermissions` (and only in Auto,
  without `--restricted`: the CLI refuses those two together), Codex CLI
  `--sandbox danger-full-access`, Cursor Agent `--force`, Copilot CLI
  `--allow-all-paths`, OpenCode `--auto`. Nothing else is unlocked: the
  wider flags each of them offers, and the ones that would send the work
  somewhere else, stay refused in every mode.

  The record keeps up: the mission ledger moves to schema 14 so a run that
  was not confined to its folder is written as exactly that. An older
  reader refuses such a mission rather than drawing it as one that stayed
  in the folder, which is the same rule that moved the number when writing
  was first allowed at all.

- **Antigravity runs end when the answer arrives.** A model that reasons
  and answers in the same step never looked finished, so the run stayed
  live and the composer kept its stop button until the idle timeout
  (Colin: "antigravity models with stop button stuck after its done with
  output"). Found by replaying his own stuck transcript.

- **The app icon is the designer's own.** The "portrait, soft fade"
  candidate: the wings run edge to edge, the antennae break the top, and
  the abdomen fades out at the bottom so the crop ends in air. Its five
  supplied sizes are used as drawn and only the three Windows also wants
  are scaled; nothing is re-rendered from an SVG, which is how a
  differently-framed picture got shipped in the first place.

- **No disclaimer under a teammate's message.** The "treated as claims"
  footer is gone with the tag that went in 0.34.1 (Colin: "teammates are
  AI, no one else adds disclaimers with their models in chat like that,
  why clutter?"). Every message is still attributed and still opens the
  conversation it reached.

## 0.34.1 — 2026-09-06

- **Cursor Agent with an effort picked ran again.** Cursor carries the
  effort inside the model id; Locust sent that id and the effort beside
  it, and every Cursor run with an effort chosen failed with "cannot be
  started with the options chosen" (Colin, on grok 4.6). The effort now
  travels as the id alone.
- **The failure names its reason.** "That runtime cannot be started with
  the options chosen. Cursor Agent takes no effort level. Nothing was
  recorded." instead of the first sentence alone.
- **A teammate's message is the way to its conversation.** The underlined
  "open the run this reached" under every relayed message is gone; the
  message itself opens the conversation it reached (Colin: "just feels
  clunky and isn't really needed").
- **No UNTRUSTED tag on an exchange.** Colin: "it's literally AI, it's
  inherently not to be supremely trusted, doesn't need to be there." The
  footer still says teammate messages are claims.
- **The app icon is Locust's again.** Colin: "it's literally showing the
  electron emblem, and it worked prior." The window had carried the right
  icon all along; Windows was drawing the taskbar icon from a Start-menu
  shortcut named "Electron" that a development run had left behind with
  the installed app's id, pointing at a bare electron.exe. Development
  runs now use an id of their own, and a packaged start removes such a
  shortcut if one exists. Also: the packaged window is handed a real
  `.ico` beside the archive rather than a path inside it, and the icon
  is the designer's dark tile — the bare white mark on a transparent
  ground was invisible on a light taskbar.

## 0.34.0 — 2026-09-06

The design agent's SURFACES-0.22 spec, built as written.

- **The fold's one line is now a trace.** "41s · thought 12s · asked 1
  subagent · 3 tool calls · 2 files" replaces "3 tool calls". The parts a
  person should notice are amber: a subagent that did not report or
  failed, a refused tool. A run that failed or was stopped reads "stopped
  at 41s", and a stopped run that changed nothing says so.
- **A subagent that never reported.** The helper row used to say "working
  on it" forever once the run had ended without the subagent's report; it
  now reads "did not report". The row names the subagent's kind ("Explore
  subagent") and carries its summary in full.
- **Subagent at work, in the sidebar.** A small mark sits beside "subagent
  working" on the teammate's card, so a glance down the list shows who is
  delegating.
- **How much of the window is used, in words.** "67% of the 5-hour window
  used, resets 10:10 PM · 53% of the 7-day window, resets Mon 3:00 AM" on
  the Claude Code row in Settings and in the route chip's tooltip. From
  80% the chip carries an amber dot and the tooltip adds "Long runs may
  be cut short."; at 100%, "This window's limit is used up."

## 0.33.2 — 2026-09-06

- **How much of the account's window a run has used.** Claude Code reports
  it while a run is still allowed ("5-hour window 35% used · resets 7:30
  PM"); Locust used to show a limit only once hit. The reading now sits in
  the route chip's tooltip and on the runtime's row in Settings, kept
  across a reload from the record.

## 0.33.1 — 2026-09-06

- **Codex's subagents show, and stay shown.** Codex records them as
  `collab_tool_call` items, which Locust filed as steps, so a run that
  spawned two agents showed nothing in the fold once they were done. They
  are tool rows now: "spawn_agent" with the ask, settled when the agent
  reports; the sidebar says "subagent working" while one runs.
- **Copilot CLI's reasoning no longer floods the thread.** Copilot 1.0.83
  streams reasoning in pieces, and each piece became an "Unhandled Copilot
  record" line. One Thinking step per reasoning now, and its tool rows
  name the file or pattern they acted on.
- **The taskbar shows Locust's icon on the installed app.** The packaged
  window takes the executable's own icon instead of a path inside the
  archive.
- **A machine with nothing installed says what to do.** Runtimes read
  NOT INSTALLED instead of UNAVAILABLE, the home screen names the agents
  Locust runs and says they appear on their own once installed and signed
  in, and the composer says the same.
- **Locust no longer vanishes on an unexpected error.** The main process
  writes it to locust-errors.log in its data folder, says so in a dialog,
  and carries on.

## 0.33.0 — 2026-09-05

Signal parity with Claude Code, measured from its own stream
(docs/SIGNAL-PARITY-2026-09-05.md has the table).

- **What a subagent is doing, as it happens.** The sidebar says "subagent
  working", the working line says which kind and what it is doing right
  now ("Explore · Reading README.md · last tool Read"), and when it reports
  back its row reads "Explore subagent · reported back · 3".
- **Claude Code's own notifications reach the thread.** A hook that failed
  ("Stop hook error occurred") is said where the run is, as a warning.
- **Settings lists a runtime's skills and agents** beside its MCP servers
  and hooks, by name, from the same folders the runtime reads.

## 0.32.3 — 2026-09-05

- **The activity fold shows the diff on Codex CLI.** That route names the
  files it changed and never sends the change, so a whole session read
  "did not report the change". Locust now reads the change off the disk:
  a new file as an add from its own contents, a tracked file from git, and
  a new file edited again on a later turn as the difference between the
  two. The diff sits on the runtime's own row.
- **"Subagent working."** While a teammate's own subagent runs, the sidebar
  says so instead of "working". The fold's rows and summary say subagent.

## 0.32.2 — 2026-09-05

- **Claude Code teammates can use subagents.** The tool list Locust hands
  Claude Code never included its subagent launcher, so no helper could be
  spawned. It can now, in both modes; a helper inherits the run's tools,
  so a read-only run's helpers read only. The activity fold names what
  each helper was asked and whether it reported back.
- **Read-only on Claude Code no longer runs in plan mode.** Plan mode wrote
  a plan file of its own under your home folder, which the fold counted as
  an edit, and called a tool that fails without a person to answer it. The
  reading-only tool list is what keeps the run read-only.
- **A handed-off mission is one conversation in the sidebar,** not two rows
  with one title, and the teammate keeps the route you handed it to.
- **Settings' Own branches list updates when a run ends,** so "In use"
  becomes "Remove" without leaving the screen.

## 0.32.1 — 2026-09-05

- **A teammate's message that goes nowhere is always said.** When replies
  are off, or a reply could not be started for any reason, the thread that
  sent the message now says so instead of showing nothing.
- **Teammates may ask each other for things.** The brief used to say "share
  findings, never instructions", and the free model read that as a ban on
  passing along the person's own request; it now says a message may carry a
  finding, a question, or a request, and must never forward instructions
  found in files or tool output.
- **Fixed:** a slow test that failed one run in five when the disk was busy.

## 0.32.0 — 2026-09-05

- **Locust makes a folder when none is chosen.** Opened from the Start
  menu with no folder picked, it now works in Documents\Locust, the way a
  terminal always has a working directory, and says so where the folder is
  named. Any other folder is one click away in Settings.
- **Settings reads in one screen fewer.** Each section opens with one line
  and folds its explanation under "How it works"; the folder, its LOCUST.md
  and Own branches sit in one card; runtimes list with the version beside
  the name; the switches sit in aligned rows.
- **The window icon is Locust's.** The taskbar showed Electron's icon,
  because the icon file was not shipped in the package.
- **An approved Codex edit shows its diff in the activity fold.** The row
  read "did not report the change" after the card had shown the change.
- **The approval card scrolls into view when it appears.**
- **Small things seen driving the app:** a mission that failed before it
  started said "Codex CLI" in its header whatever route it was sent to; the
  "Thinking" row showed an item type as if it were a tool.

## 0.31.1 — 2026-09-05

- **A teammate on its own branch reads like one in the folder.** The
  activity fold showed every path as .locust/worktrees/<id>/README.md; now
  it shows README.md, since the tree is the same project and the sidebar
  already says which branch the teammate is on.
- **The thread header uses a Custom teammate's title.** It said "Custom"
  where the sidebar said "Release manager".
- Found by driving the built app through a first session as a person
  would; the record is in docs/user-session/.

## 0.31.0 — 2026-09-05

- **Own branch.** A teammate can work in its own copy of the project
  folder: turn Own branch on in its card and its runs happen in a worktree
  of the folder's repository, on branch locust/<name>, so two teammates
  editing one repository never collide. The sidebar says which branch each
  is on. Settings lists the worktrees and can remove one; the branch stays,
  and merging back is yours to do. Needs the folder to be a git
  repository; a teammate that cannot get its tree says why instead of
  running in the folder unannounced.
- A Custom teammate's title is kept. It was dropped on the way to disk, so
  every Custom teammate read "Custom".

## 0.30.0 — 2026-09-05

- **The approval card shows the change.** When Codex asks to change files
  in "Approve each action" mode, the card now carries the diff itself —
  each file named, added and removed lines counted, the lines drawn with
  the same viewer the activity fold uses — instead of a summary of what
  it was told. Nothing changes until you approve, as before.

## 0.29.0 — 2026-09-05

- **One instruction file for the whole team: LOCUST.md.** Put a LOCUST.md
  at the root of the project folder and every teammate, on every runtime,
  is given it before each mission — the file they all read in common,
  beside each runtime's own CLAUDE.md, AGENTS.md or rules. Read fresh at
  every start, so an edit lands on the next mission. Bounded at 200 lines,
  and the brief says when the rest was cut. Settings shows whether one was
  read and how much of it.

## 0.28.1 — 2026-09-05

- **Two more things reach the desk while you are away.** A teammate that
  ends a run with a question card, and a run that stops at its account's
  limit, now show a desktop notification like an approval does — only when
  Locust is not the window in front. Clicking it brings Locust forward.

## 0.28.0 — 2026-09-05

- **Settings shows what each runtime has set up for itself.** Under every
  runtime: its MCP servers and its hooks, by name and event, read from the
  runtime's own configuration files (Claude Code's settings and MCP files,
  Codex's config.toml, Cursor's, OpenCode's and Copilot's). Names only, so
  no command line or secret reaches the screen; the tooltip names the files
  read. Locust adds none of its own and changes nothing there. A runtime
  with nothing configured says so in words.

## 0.27.6 — 2026-09-05

- **A runtime's own helper has its own row.** When Claude Code or OpenCode
  starts a sub-agent for itself, the activity fold now says "asked 1
  helper" apart from the tool calls, and the row says what the helper was
  asked and whether it reported back. What the helper did inside is not
  reported by the runtime, so nothing is invented about it.

## 0.27.5 — 2026-09-05

- **A runtime slow to answer reads CHECKING, not UNAVAILABLE.** Discovery
  ran once at launch, so a runtime whose first probe took too long (Claude
  Code on a cold start) stayed marked unavailable all session. Now an
  installed runtime that did not answer in time is tagged CHECKING, Locust
  asks again three times fifteen seconds apart, and once more whenever the
  window comes back into focus, so a sign-in done elsewhere shows without a
  relaunch. UNAVAILABLE is kept for a runtime that is not on the machine.

## 0.27.4 — 2026-09-05

- **The home screen, tightened after a design review.** The count says how
  many runtimes are connected, without the two planned ones in the
  denominator; those are named once under the panel as coming soon. The
  privacy claim is made once. The instruction to pick a teammate is the
  sidebar's alone. The mark card is smaller. Runtime names sit against
  their dots. Cursor's build stamp shows its date, not its commit hash.
- **The permission mode looks like the control it is**: boxed like the
  route and effort chips, with a shield and a chevron.
- "no effort" reads as "effort · fixed" (or "effort · default").

## 0.27.3 — 2026-09-05

- **The logo takes you home.** Click the Locust mark in the sidebar to
  return to the home screen from anywhere.
- **Memory lives in Settings.** The sidebar row is gone; Settings has the
  mode, the count, what is waiting for you, and Open memory (Ctrl 5).
- **What a conversation taught the team folds like tool activity.** One
  quiet line — "Wren remembered 2 things" — with the lines a click away,
  instead of warning-coloured notices at the bottom of the thread.

## 0.27.2 — 2026-09-05

- With no folder chosen, the title bar shows the build (Locust 0.27.2)
  instead of repeating what the composer's folder chip already says, and
  that chip reads in the usual soft gray rather than amber.

## 0.27.1 — 2026-09-05

- **Locust opens on the home screen.** Launch showed the newest finished
  conversation instead of the home screen with the connected runtimes and
  the folder. Now a conversation is put on screen at launch only if it is
  still running; otherwise the home screen is what you see, and the
  sidebar has the chats.

## 0.27.0 — 2026-09-05

- **Your team remembers.** Teammates keep a shared memory per project
  folder, plus a smaller set marked everywhere — the way Claude Code and
  Cursor do, managed from Locust. A teammate writes one by ending a reply
  with it; every teammate in the folder reads what is kept, with who wrote
  it and where. The Memory screen (Ctrl 5, or the row at the top of the
  sidebar) lists every memory with who, where, and the conversation it came
  from: edit, switch off, remove, or write one yourself. Settings chooses
  what happens when a teammate writes a memory: keep it and say so in the
  conversation, ask you first, or off. Nothing leaves this machine.

## 0.26.1 — 2026-09-05

- **Editing a schedule keeps what you set.** Pressing the choice a routine
  already has (Daily, or Every few hours) no longer resets its time or its
  hours. Found by driving the edit path after 0.26.0 shipped.

## 0.26.0 — 2026-09-05

- **Routines can run on their own.** Saving or editing a routine now offers
  a schedule: every few hours (1 to 24, counted from its last run) or daily
  at a time. A scheduled routine starts exactly as if you pressed Run — on
  its teammate's route, recorded as started by the routine — and the Team
  card says the rule and the next run. It runs only while Locust is open
  and only when its teammate is free; a run missed while Locust was closed
  happens once, when it is next open, not once per missed interval. A start
  that fails is tried again an hour later, not every minute.

## 0.25.0 — 2026-09-05

- **Rooms tell you when something happened while you were away.** A teammate
  moving the board, or the last teammate answering a post, shows a desktop
  notification — only when Locust is not the window in front. Changes to
  one room are gathered for two minutes and said once, newest last, so three
  teammates finishing together are one thing to read, not three.

## 0.24.0 — 2026-09-05

- **Tasks in a room.** Every room has a board: a task is a line of text, an
  owner, a state (open, in hand, done) and the conversation that last moved
  it. Add, assign, finish, reopen or remove tasks from the room. Teammates
  move the board themselves by ending a reply with a task block — they are
  told the board and the block with every post — and the room says what
  they did.

## 0.23.0 — 2026-09-05

- **Rooms.** Make a room out of some teammates (Ctrl 4, or the Rooms section
  in the sidebar once you have one). Post to it and every teammate in it
  answers in their own card, each on their own runtime and model; each card
  opens the conversation it came from. A post starts an ordinary mission per
  teammate, so everything you already know about missions applies.
- **A blank window after switching the route on a finished conversation.**
  With a teammate picked and their finished conversation open, choosing a
  different runtime for the next message could throw during render and leave
  nothing on screen (0.21.6 to 0.22.1). Fixed; found by the room's own smoke.

## 0.22.1 — 2026-09-05

- **The sidebar no longer promises a teammate will pick up a message you
  are sending to nobody.** With no one picked it says so; once someone is
  picked it names them. The first-run footnote said the same wrong thing.
- **Nobody is drawn as chosen until you choose them.**
- **Settings' READY tags are green, not lime.** Lime means something is
  happening right now.

## 0.22.0 — 2026-09-05

- **See the exchange.** When teammates are talking to each other, the
  conversation shows who is in it and on what model, how many automatic
  replies it has used of your budget, what every run in it has cost so far,
  and a Stop that halts all of them at once.
- **The budget is yours.** Settings → Teammates now has "Automatic replies
  per exchange" as fixed steps (1 to 12). Six was a constant; now it is a
  number you chose, and the exchange line counts against it.

## 0.21.6 — 2026-09-05

- **A runtime's usage limit survives a restart.** Settings said AT LIMIT, a
  reload said READY, and nothing had changed. The ledger knew; now the window
  asks it on the way up.
- **Reply on another model after a run stopped.** Switching provider after a
  failure used to start a stranger with no memory of the task. The next turn
  now starts on the new runtime from the old run's checkpoint, briefed on what
  was done and what was left unsettled, with your reply as its latest
  instruction. The composer says so before you send.
- **Edits a runtime never mentioned are still shown.** When a run allowed to
  write ends, Locust compares the working tree (git) with how it was before,
  and every changed file no tool named becomes an *observed on disk* row.
- **A conversation with nobody, and Assign to a teammate later.** From the
  home screen with no one picked, a message is just a message. Right-click
  the conversation to hand it to a teammate.
- **Headings and bold render in replies.** `### Summary` and `**like this**`
  no longer arrive as punctuation. Links stay labels on purpose.
- **Settings tells the truth about limits.** Hand off, continue elsewhere,
  and the automatic fallback that is deliberately not built.
- **The route picker's search no longer floods.** One letter gave 92 rows;
  each runtime now shows twelve and says how many more match.
- Small windows: no stray scrollbar or clipped text in the collapsed sidebar.

## 0.21.5 — 2026-09-05

- **Your teammates work in a folder you choose.** Opened from the Start menu,
  Locust took its own install folder as the workspace, so every teammate was
  reading and editing inside `AppData\Local\Programs\Locust`. Antigravity
  refused outright and OpenCode auto-rejected its way out and failed. Now the
  install folder is never a workspace: Locust uses the folder you last chose,
  and until you choose one it refuses to start anything and says so. The
  folder lives in Settings, on a chip beside the permission mode, and in the
  title bar.
- **The permission menu no longer folds under the window.** It opens from a
  control at the bottom edge, and a stylesheet rule left behind by a deleted
  overflow menu had flipped it to open downward. Two pixels of a 305px menu
  were on screen.
- **A runtime's own words arrive readable.** Failure cards showed the terminal
  colour codes around them as little empty boxes.
- **Right-click a teammate** to message, edit, or remove them. Removing says
  how many routines go with them.
- **A refused delete says so.** The message only appeared inside the
  conversation you were looking at, so refusing a delete from the sidebar
  reported nothing anywhere and looked like the menu doing nothing.
- **Claude Code's refusals are reported.** When it is not permitted to run
  something it stops quietly; the run now says which tool was blocked.
- **First run rebuilt.** The mark sits in its own card, the runtimes are one
  panel in two columns rather than a stack, connected ones come first, and
  the count on the screen agrees with the count in the sidebar footer. The
  route and effort now read as a pair of boxed controls.
- **The sidebar's empty message no longer wraps to one word per line** on a
  narrow window.

## 0.21.4 — 2026-09-05

- **Delete in the mission header removes the whole conversation.** It removed
  only the turn whose id the header carried, leaving the rest of the thread
  you were looking at. The sidebar's Delete had the same bug and was fixed in
  0.21.0; this was the other half of it.
- **The header names the model, not just the runtime.** This app exists to
  put two models on the same work, and two missions from different models
  read identically once the composer had moved on.

## 0.21.3 — 2026-09-05

The welcome screen, as the design pass drew it.

- **One greeting, not two.** The wordmark already says the name in the
  largest type on screen, so the headline under it was a second voice saying
  less. It survives only where it is information: nothing here can run, and
  saying so is the screen's whole job.
- **The mark, not a box holding the mark.** A bordered card wrapped a logo
  that already sits in the sidebar 40 pixels away.
- **Two claim lines, not three.** Once something is signed in, the roster
  line says discovery ran and what it found, so the line repeating that in
  other words steps aside. It stays when nothing is ready, where it is the
  only account of what happened.

## 0.21.2 — 2026-09-05

- **The box you type in is on screen when the app opens.** On a 1280x860
  window the welcome screen's eight runtime rows pushed the composer below
  the fold, while the copy said "describe a mission in the box below" — the
  sentence was true and the layout made it a lie. The welcome now keeps to
  the space it has and scrolls inside it, so it can never push the composer
  anywhere. Measured: 16 pixels above the edge, where it used to be 395
  below.
- **The runtime list is a count you can open.** It reads "5 of 8 runtimes
  signed in under your own accounts", and opens to the full list. It starts
  open when nothing is ready, because then the list is the whole point of the
  screen, and closed when something can run, because then the point is to
  type a mission. The sentence every signed-in runtime repeated is said once,
  above the list, so each row is a name, a version and its state.

## 0.21.1 — 2026-09-05

The rest of the design pass's objections.

- **A queued message can be edited.** While one is waiting the box is
  disabled, so fixing a single word meant discarding the sentence and
  retyping it from memory. Edit lifts it back into the box.
- **Every held message says why it is held**, and the reason now sits under
  the message where a caption belongs rather than competing with it. The one
  state that described the button instead of the reason is gone, and cannot
  come back: a queue that is ready to send has already sent.
- **"Add a step" stays and says why** at a routine's twelve-step limit
  instead of disappearing, which read as a broken dialog. The Steps label
  carries the count, so the limit is visible before you meet it.
- **First launch is calmer.** Its READY tags are green rather than lime.
  Lime means happening right now, and nothing on a first launch is
  happening; five lime elements leave the quietest screen in the app.

## 0.21.0 — 2026-09-05

- **You can scroll a long conversation again.** The thread used a layout rule
  that pushes content out of the top of a scrolling box, and an overflowed
  top cannot be scrolled to — so past about a screenful, everything above the
  newest work was unreachable. It now uses spacing that collapses when there
  is no room to spare, so the scroll always starts at the first message.
- **Delete removes the whole conversation.** A sidebar row stands for every
  turn of one conversation, but Delete removed only its last turn, so the row
  stayed on screen and the menu read as doing nothing.
- **Plan is a permission mode, not a switch beside one.** It could only ever
  be on together with a read-only mode, so the composer was asking the same
  question twice in two shapes and the two could disagree. Plan now sits in
  the mode menu with the others and states its consequence there: answers
  with the steps it would take, and changes nothing. It is offered exactly
  where read-only containment is real, and withheld with the runtime's own
  reason where it is not. "Build this plan" is an ordinary mode switch now
  rather than a hidden state change.
- Each message on an exchange card can open the run it reached, so a
  teammate's reply is reachable from the conversation that asked rather than
  by scrolling the sidebar. Offered only where the record shows something
  received it.

## 0.20.1 — 2026-09-05

Three things Colin found in an evening of real use.

- **A teammate's briefing is no longer shown as your own message.** When one
  teammate answers another, the host writes that run a briefing — *"end with
  one <locust-share to="Wren"> block... do not use a <locust-ask> block
  here"* — and the thread was drawing that whole paragraph in the place your
  message goes, as the most prominent text on screen. It now shows the
  message that caused the turn, the way the mission list already did, and
  shows nothing at all when the record no longer holds it. The rule the
  thread was written with, and had been breaking: a run the host briefed is
  never drawn as a person's words.
- **The box empties the moment you send.** It was waiting for the host to
  answer first, so your words sat in the box beside the bubble for the whole
  "Starting…" second or two and read as lag. If a send genuinely never
  happens, the words come straight back.
- **A teammate who never writes back now says so.** Booty asked Wren a
  question; Wren answered in its own conversation without a reply block, so
  nothing came back and Booty's thread showed nothing — which reads as the
  message never arriving. A meeting has always said who stayed silent; a
  one-to-one exchange now does too, and says the answer is in that
  teammate's own conversation.
- **The message box says "Write a message", and nothing more.** Three of its
  lines used to restate the permission mode — "it may edit files in this
  workspace" — which the mode control says in two words directly underneath.
  Every other line it can show survives, because each says something no
  other part of the screen does: a runtime still being looked for, one that
  needs signing in, a route this build cannot run, and what happens to what
  you type while a mission is working.
- **The composer no longer gets clipped by the window edge.** It could give
  up height when the window was short, so its bottom row — route, effort,
  swarm — folded under the edge. It now keeps its height, and its controls
  wrap rather than running off the end when a model id is long.

## 0.20.0 — 2026-09-05

The two habits people bring from other agent tools, and miss first.

- **Say the next thing while a teammate is still working.** The box used to
  be dead during a run, so the only way to add an instruction was to stop
  the work. Now it stays usable: what you type waits, the screen shows it
  waiting, and it goes as the next turn the moment that run **completes**.
  If the run failed, was stopped, or never finished, it is held instead,
  with the reason and a Send now button — the next instruction assumes the
  last turn happened, and sending it into a turn that did not is how you
  end up building on work nobody did.
- **Plan first.** A control beside the mode: the run answers with the steps
  it would take, numbered, and changes nothing. The thread then says so in
  its own words and offers **Build this plan**, which starts the doing turn
  of the same conversation with edits allowed. Offered only in a mode whose
  sandbox already refuses writes, and the host checks that again rather
  than trusting the window — a plan that could edit your files is a promise
  the app cannot keep. Cursor Agent on Windows has no sandbox that can hold
  a run read-only, so plan first is not offered there, and the control says
  that rather than telling you to switch to a mode you cannot pick.
- **A turn that ends without a reply now says so.** Found by Colin watching a
  live test: a follow-up finished cleanly, spent tokens, recorded its
  reasoning and wrote nothing back. The thread showed the message, then
  blank space, under a header reading "completed" — which reads as the app
  losing the answer. It now says the runtime finished and wrote nothing,
  that nothing was changed, and that sending again usually works.

## 0.19.0 — 2026-09-05

- **Routines: teach a teammate a job once, then hand it back any time.**
  Right-click a finished conversation and choose *Save as routine*. The
  dialog arrives already filled in with what you typed on each turn, in
  order, so you are editing rather than writing. Press Run on the Team
  screen and the teammate replays it: step one starts, and each later step
  starts only when the one before it **completed**. A step that fails, is
  stopped, or is interrupted ends the routine there and says which step and
  why, rather than building the next step on work that never happened.
  - A routine belongs to a teammate and replays on the route it was learned
    on, so one saved read-only stays read-only.
  - Corrections are the point: edit the name or any step, and the next run
    uses the corrected version. Editing cannot move a routine to another
    teammate, change what it runs on, or lose where it came from.
  - Every replayed run is recorded as one, with the routine and the step
    number in the mission's own header, so a shared record never reads as
    though a person asked for it.
  - The sidebar says which step is running, read from the runs themselves,
    so the label cannot outlive the work.
  - **What this is not:** a teammate still cannot watch you work outside
    Locust. It has no view of your editor, browser or terminal. It can only
    learn from work it did with you, which every mission already records.
    That is the honest version of the feature, and it is most of the value.

## 0.18.3 — 2026-09-05

Found by using the app as a new person would, in a fresh profile, with the
screen at every step kept and read afterwards.

- **A runtime that just ran out of quota no longer says READY.** Two missions
  in a row failed on Codex's usage limit and Settings, the welcome list and
  every Codex row in the route picker still read READY — which means signed
  in, not able to run. Until a run on that runtime completes, those rows now
  say AT LIMIT and carry the runtime's own sentence, reset time included.
  The row stays pickable: the limit is your account's and lifts on the
  provider's clock.
- **One failure, said once.** A quota failure was drawn three times in a row:
  the limit card, the runtime's red error line, and the run's own failure
  card, each carrying the same sentence. The limit card is the one that
  names it; the other two stay out of its way. A slow-down warning never
  hides a real failure reason.
- **The idle teammate's sentence now matches the mode.** It said "Nothing is
  changed unless you pick a mode that allows it" above a composer whose
  default is Accept edits, so a fresh Research teammate promised read-only
  while the footer said it may edit the workspace. It now says what the
  current mode does.
- **A reply keeps its line breaks.** Asked for "every file, one per line",
  the teammate answered with one per line and the thread drew them on one
  line, which read as the teammate ignoring the request. Checked against the
  record: the newline was there; the screen dropped it.
- The Missions list says "checkpoints", not "ck". The teammates screen is
  titled Team, as its button already was. A model with one effort level no
  longer reports "1 effort levels".

## 0.18.2 — 2026-09-05

- **A teammate answering a teammate no longer asks a person who isn't there.**
  Since 0.17.0 a run that reaches a fork stops and asks you with a decision
  card. A relayed run — one teammate replying to another — still had that
  instruction in its prompt, and one in three test exchanges used it: the
  recipient (rightly) wanted context before acting on a message it could not
  verify, asked for it with a card, and the card sat in a thread nobody was
  watching. No reply went back, and the exchange ended in silence. The relayed
  brief now says there is no person in the exchange and that any question
  goes in the share block to the teammate who asked. Meetings get the same
  line.
- **When a teammate's reply arrives, the thread still shows what was sent.**
  A reply that comes back on its own opens the next turn of the thread that
  asked, and that turn was rebuilt without the message it had sent — so the
  thread showed the answer and not the question, and read as though the
  teammate had answered you. The person-typed follow-up had the same fix on
  2026-09-04; this is the other path.

## 0.18.1 — 2026-09-05

- **A read-only Copilot or OpenCode mission could silently have write access.**
  Both take the request as a command-line argument, reached through `cmd.exe`,
  and `cmd.exe` stops reading a command line at the first newline. Any
  multi-line request — every teammate briefing is one — dropped every flag
  after it: the JSON output the app reads, and the flag that made the run
  read-only. The run then ran in Copilot's human mode, exited 0, and the thread
  said it "ended without a terminal result record". Fixed by running what the
  npm shim wraps directly — `node` plus the script, or the native `.exe` —
  with no shell in between, which also removes the 8,191-character ceiling.
- **A request too long for a Windows command line is refused with the reason**,
  naming the limit, the actual length, and the runtimes that read from input
  instead — rather than failing with nothing on screen to explain it.
- **The record now says what was actually run.** Each mission's header carries
  the executable and flags, with the request itself replaced by a marker so it
  never rides along in a shared ledger. This is how the bug above was found.
- The waiting line's clock counts from the start of the turn instead of
  restarting on every event, so it climbs rather than looking like a loop.
- Launch failures report their real reason instead of "could not be started
  safely" for everything.

## 0.18.0 — 2026-09-05

- **A mission the app stopped in the middle of can be picked back up.** The
  ledger has written checkpoints since the beginning and the receipt has
  reported them; what never existed was the way to act on one, so an
  interrupted mission was a record you could read and nothing else. It now
  offers to resume from its last checkpoint, and the new run is told what had
  finished, what had not, and what to verify before building on it.
- **It says which of three things is true, rather than showing a button that
  might not work.** If every recorded action reported an outcome, it offers a
  plain resume. If something started and never reported back, it offers the
  resume *and names those actions*, so you can go and look before saying go.
  And if the ledger itself came back incomplete, it refuses and says why —
  continuing from a record the app cannot vouch for would build on work it
  cannot describe.
- **Closing the app mid-run is what this is for; pressing Stop is not.** A run
  you stopped on purpose is not offered a resume, because undoing your own
  decision is not the app's to suggest.
- A teammate's question can no longer be a yes/no. That is a permission
  request, and there is already a card for those.

## 0.17.0 — 2026-09-05

- **A teammate can ask you which way to go, instead of guessing.** When a run
  reaches a real fork — two defensible ways to do what you asked, where picking
  wrong means undoing work — it can stop and put the question to you as a card
  with the options as buttons, each labelled with what the runtime says it
  costs. Your answer starts the next turn.

  This is deliberately **not** the approval card. An approval asks *may I do
  this thing I am about to do*; the agent has already decided. This asks
  *which of these should I do*, before anything is done. Until now an agent at
  a fork had exactly one move — pick, and carry on — and you found out
  afterwards from a diff.

  No option is marked as recommended: the card exists because the model
  reached a decision it should not make alone, and quietly nominating a
  favourite would make it anyway. The card also says plainly that you can
  ignore the buttons and just reply.
- **The card never claims more than it knows.** The design's line was "paused,
  nothing changed" — but a run permitted to edit may well have changed files
  before it asked. So it says *nothing was changed* only when the mode made
  writing impossible; otherwise it says work is kept, or that the run could
  edit, and claims nothing.
- Every mission now tells its runtime how to ask, and — as importantly — when
  not to: not for anything it can settle by reading the workspace, and never
  as a way to ask permission to continue.

## 0.16.6 — 2026-09-04

- **A teammate's automatic reply is no longer named after machine
  instructions.** When one teammate writes to another, the host starts the
  recipient's run with a prompt the host wrote — *"Wren (Code & Migrations)
  sent you a message... end with one `<locust-share>` block"* — and that
  sentence was appearing as the NAME of a mission, beside conversations you
  actually started. It is now named by the message that caused it: *"Wren
  asked: Please reply with the passphrase."* If the workroom no longer holds
  that message the briefing still shows, because an invented title is worse
  than an ugly true one.
- **The ledger records who started a run.** Until now every mission in the file
  looked like something a person asked for, because every mission was. Nothing
  said otherwise when the app started one itself, so it could only present its
  own work as yours.
- A waiting line no longer says **Working** directly above an approval it is
  stopped on, while the header says *waiting on you*.

## 0.16.5 — 2026-09-04

- **You can see both halves of a conversation between two teammates.** Ask one
  of them to message another and the thread showed the reply and never the
  question — so it read as though the second teammate had answered *you*. The
  message asking is written on an earlier turn than the answer, and the thread
  only ever drew the newest turn's. Every turn's now shows, filed against the
  turn it happened on.
- **A short exchange opens where it sits.** Two messages is something you read
  in place, not a toggle to find. Longer ones stay folded and now show their
  first line instead of only counting themselves.
- **Pressing send does something immediately.** The working line appeared only
  once the runtime reported its first step, so for a CLI that has to launch a
  process the thread sat blank for seconds and the bounce looked late. It was
  not late — there was no line for it to be on. A live run always shows one
  now, and it says what is true: with no step reported it names the wait rather
  than inventing a step.
- **The `...` shows up where you are actually waiting.** It used to mean "a
  reasoning step is open", which most runtimes never report, so it almost never
  appeared. It now means waiting on the model with nothing to show yet.
- **The composer lets go of what you sent.** A start the host refused left your
  text on screen twice — as a failed turn and still in the box — which read as
  though nothing had been sent. If the bridge is missing the text stays, because
  then the box is the only copy of it.
- **The window remembers its size and position.** It also refuses to reopen onto
  a monitor that is no longer plugged in, which would put it where you cannot
  see or drag it.
- The app icon drops the black tile and keeps the ink.

## 0.16.4 — 2026-09-04

- **A new app icon.** The mark is a fine engraving, and downsampled to the
  sizes a taskbar actually uses it averaged to grey — under 2% of the tile
  carried solid ink at 32px, so it stopped reading as a locust. The new one is
  the same artwork, thickened and zoomed so the wings reach the edges: 43% at
  32px, and the `.ico` now carries every size Windows picks from rather than
  making it scale one.
- **The window opens at a sensible size.** It was a flat 1480x940 — most of a
  laptop screen. It now takes a fraction of your display and caps there.
- The sidebar's second button reads **Team**.

## 0.16.3 — 2026-09-04

- **A short conversation sits on the composer** instead of hanging in mid-air
  above 70px of nothing. It grows upward out of the box you type in, the way
  every chat does.
- **The sidebar footer cannot bleed past the rail.** Its buttons are a grid
  that can shrink rather than a row laid out by content, and the connection
  count has its own line: `6 runtimes connected`, with a settled dot.
- **The window icon uses the 512, not the 256 beside it.** Windows scales from
  whatever it is handed, so it was downsampling a downsample.
- The visual pass from 2026-09-03 is applied throughout: code blocks, lists,
  link labels, the read-only rerun note, and the roster card.

## 0.16.2 — 2026-09-03

- **The routes you move between sit at the top of the picker.** Recency
  already ordered models within a runtime, which helps when you stay on one
  and does nothing for the move this app exists for — putting two models on
  the same work. The other runtime's group sat below six rows of the one you
  were on, in a list showing less than half its height. A Recent group now
  carries the routes actually used, across runtimes. It appears only once
  there are two of them (a single recent is the route you are already on) and
  never lists a model its runtime has stopped offering.

## 0.16.1 — 2026-09-03

- **Opening Locust in a project shows that project's work.** It opened on the
  most recent mission anywhere, so starting it in a new folder greeted you with
  a conversation from a different one, and the sidebar listed that folder's
  missions too. The workroom now keeps to the folder it was launched in; the
  Missions screen is still the whole archive, which is what it is for.
- **Two runtimes were recording a random id for the folder they ran in.**
  Codex missions hashed the workspace path, so they could be matched back to a
  folder; the app-server and Antigravity paths minted a fresh id each time,
  which looks identical in a receipt and means the opposite. All three now use
  the same derivation, and the path itself never enters the ledger.

## 0.16.0 — 2026-09-03

- **The Teammates screen says what a teammate has been doing.** It could tell
  you a teammate existed, their route, and how many missions they owned —
  which answers "who is on my team" and not "what have they been up to". Each
  card now carries when they last ran, what their work has cost, and their
  newest missions, and clicking one opens it. A teammate who has never run
  reads `never`, `not reported`, `not set yet`, because a card that filled
  those with zeroes would be claiming things nobody measured.

## 0.15.8 — 2026-09-03

- **A retrying mission stops looking frozen.** Against a dead endpoint Codex
  retries five times across several minutes and reports each attempt. Every
  one of those notices arrives before the first tool runs, and the thread
  dropped everything that arrived that early — so the mission sat reading
  "running" with an empty thread while the runtime was working. Setup chatter
  still stays hidden; trouble with the run itself no longer does.
- **A mission from another day says which day.** The marker read `started
  12:25 AM` with no date, which is unambiguous only until tomorrow.
- **A teammate's card shows the mode they actually ran in.** It printed
  `read-only` for everyone, whatever they had run in — a fact the card never
  had, and simply false once a runtime could edit.

## 0.15.7 — 2026-09-03

- **Claude's activity rows say what they touched.** They read `Read done`,
  `Glob done`, `Write failed` — the tool and nothing else, so the card could
  not tell you which file was read or written. A Claude tool's input arrives
  after the call opens, streamed as JSON, and nothing had picked it up from
  the finished block. Rows now read `src/format.js Read`, `src/cli.js Read`,
  `src/format.js Edit`.
- **The sidebar stopped pointing at a box that is not there.** Its empty state
  said "Describe one below" on the Teammates and Settings screens, which have
  no composer.

## 0.15.6 — 2026-09-03

- **Claude Code really can edit now.** 0.15.5 said it could and it could not:
  two places still forced read-only before the mode reached the process. The
  host coerced Claude's sandbox to `read-only` outright, and the Claude branch
  built its command without passing a sandbox at all. The receipt claimed
  `workspace-write` while the run was in plan mode and answered "I don't have
  a Write tool available in this session" — the record and the process
  disagreeing, which is the one thing a receipt must never do. Verified from
  the ledger this time: a mission recorded `runtime: claude` and
  `sandbox: workspace-write`, and Claude created a new file.
  **0.15.5's note that this was "verified live" was wrong** — the run used to
  verify it was Codex, not Claude. Corrected here rather than quietly.

## 0.15.5 — 2026-09-03

- **Claude Code can edit now, like it does in its own app.** It was launched
  `--permission-mode plan` with a three-tool reading list on every mission,
  whatever the composer asked, so it could only ever read. Accept edits now
  sends `--permission-mode acceptEdits` with the editing tools and Bash named,
  and a Claude mission can change files and run the tests it just changed.
  Read-only keeps exactly what it had. Both stay `--restricted`, so your own
  Claude settings never leak into a mission and the tool list is explicit
  either way.
  (0.15.4 had "fixed" the composer-says-one-thing-header-says-another problem
  by removing Accept edits from Claude Code. That was the wrong repair for the
  right complaint: the mode now decides the arguments.)
- **The sidebar footer stopped wrapping.** Adding the Missions and Teammates
  buttons squeezed the connection count until "6" sat above "connected".

## 0.15.4 — 2026-09-03

The rest of what using the app turned up. Notes in
`docs/USING-IT-2026-09-03.md`.

- **Asking a teammate to talk to another teammate now reaches them.** Asked to
  review some tests and ask a colleague whether they agreed, a teammate wrote
  the colleague's name into its reply and stopped — and the colleague never
  ran. Nothing had told it that naming someone in prose does not reach them,
  and the briefing opened with wording that discouraged a hand-off exactly when
  you had just asked for one. Both are now said plainly.
- **Claude Code offers Ask only, because that is all it can do.** The composer
  said `Accept edits` while the mission header said `read-only` on the same
  screen: Claude Code is always launched restricted to reading. The composer
  now never shows a mode the chosen route cannot honour.
- **Lists and links render.** Bullets and numbered lists were collapsing into
  the sentence around them, and `[label](target)` showed its raw brackets with
  a full absolute path in the middle of a line.
- **Paths read the way you write them.** An activity row showed a long temp
  path with the filename cut off; inside the workspace it is now
  `src/streak.js`. Outside it the full path stays, because there the location
  is the information.

## 0.15.3 — 2026-09-03

Found by using the app on a real project rather than testing it. Notes in
`docs/USING-IT-2026-09-03.md`.

- **A Claude Code answer no longer appears twice.** Claude streams its reply
  and then sends the finished message to replace what streamed; the replace
  targeted a fixed block while the text had arrived in a different one, so the
  whole answer rendered a second time underneath itself.
- **Shell rows show the command, not the thing that ran it.** Every command was
  displayed as `"C:\Windows\...\powershell.exe" -Command "..."`, and since a
  row is one line wide, all you could read was the same truncated path. They
  now read `npm test`, `node --test`, `git status --short`.
- **A Codex edit lists one row per file again**, instead of one row with every
  path run together saying the change was not reported.
- **The first screen stops asking for something already done.** It said
  "Connect a runtime to start working" above six runtimes marked READY; when
  something can run it now says so and points at the composer.

## 0.15.2 — 2026-09-03

- **Fixes a Codex regression 0.15.1 introduced.** Making PATH win meant
  reaching npm's `codex.ps1`, and that shim cannot take Codex's own arguments
  — the bare `-` that sends the prompt on stdin makes PowerShell reject the
  whole call — so every Codex mission failed for anyone whose Codex came from
  npm. Windows itself runs the `.cmd`, and now so does Locust. **If you are on
  0.15.1 and Codex stopped working, this is why; install this one.**
- **Replies render as written.** Fenced code blocks are code blocks and
  `inline code` is inline code, instead of one flat paragraph with the
  backticks still in it. This mattered most exactly where it was worst: a
  read-only run cannot edit the workspace, so it pastes the patch into its
  answer, and a diff with every newline collapsed is the one reply nobody can
  read. Long lines scroll inside the block rather than stretching the thread.
- **A new teammate starts on Accept edits.** Under Ask, "add a discount
  function" was refused by the sandbox and the model pasted its patch into the
  reply instead, with nothing saying the mode was why. Ask is still one click
  away, and a teammate you have run keeps whatever they last ran on.
- **A read-only run that answers with code offers to run again with edits
  allowed** — one click, instead of changing the mode and retyping. It
  appears only when the reply actually carries code, and it is an offer, not
  an error: the run did exactly what its mode permits.

## 0.15.1 — 2026-09-03

- **Locust now runs the runtime you installed, not an older copy it found
  somewhere else.** It searched every likely install directory for a `.exe`
  before it looked on PATH for a shim — and a tool installed from npm lands
  as a `.cmd` and a `.ps1`, never a `.exe`. So a runtime you installed or
  updated could be ignored in favour of an older copy sitting somewhere the
  app had guessed, with nothing on screen to say which one was running. What a
  terminal would run now genuinely wins.

## 0.15.0 — 2026-09-03

- **Updates now actually install.** "Install and restart" quit the app and
  brought it back on the same version; the update it had downloaded was never
  applied. If your copy has been stuck on an old version with "ready to
  install" in Settings, this release fixes that. It cannot fix itself from
  inside the stuck copy, so this one time: quit Locust, and run the installer
  from the download page, or the one waiting in your updater folder.
- **Codex edits name their files.** A change Codex made showed a row with no
  path and "did not report the change"; a two-file change said "Edited 1
  file". Every changed file is now its own row, and the count is files.
- **The window is titled by your folder**, not "Local workspace".
- **Missions and Teammates are one click away** in the sidebar footer, with
  their shortcuts in the tooltips.
- **A failure now says what the runtime said.** Every failure card showed the
  app's own sentence — "Codex invocation did not complete successfully" —
  and threw away the runtime's explanation, which the ledger had been
  recording all along. Two runs lost this way turned out to be a folder Codex
  refused to work in and a Cursor account out of capacity; on screen both read
  as the same shrug. The runtime's own last word is now on the card, and
  capacity exhaustion is said in English instead of as `resource_exhausted`.
- **Codex missions work in a folder that is not a git repository.** Codex CLI
  refuses to start outside a repo unless asked not to check, so a mission in a
  plain folder died in half a second before the model was ever reached. Locust
  decides what a run may touch itself — read-only or accept-edits, an approval
  gate, and a recorded diff of every write — so it now asks Codex to skip that
  check.
- **A reply after a failed run stays in the same conversation.** Being the next
  turn and resuming a runtime's session are different things, and the second
  was gating the first: replying to a run that failed before its runtime
  started opened a second sidebar row and dropped the turn above it from the
  screen. The conversation now continues either way, and when the runtime has
  no session to resume the thread says the model started without the earlier
  messages rather than letting you assume it remembers.

## 0.14.0 — 2026-09-03

- **What a run cost.** The receipt, the inspector and a new column on Missions
  say what each run cost in the runtime's own unit: dollars for Claude Code,
  premium requests for Copilot, tokens in and out for the rest, with a total
  across priced runs. A runtime that reports nothing is shown as exactly that,
  never as free.
- **Approvals find you.** If a teammate needs an approval while Locust is
  behind another window, you get a notification naming the teammate and the
  action; clicking it brings Locust forward. Nothing else notifies.
- **Custom roles mean something.** Pick Custom and say what the teammate does.
  Those words show beside their name and are what their runtime is told.

## 0.13.0 — 2026-09-03

- **Antigravity, as an experimental route.** If Google's Antigravity is open
  with your folder, Locust can hand its agent a mission on the Flash, Pro or
  Flash Lite tier, watch the work land in the thread (tool calls, the final
  answer), keep the receipt, and continue the conversation on a reply. It is
  tagged EXPERIMENTAL in the picker because it drives an interface Antigravity
  never published; it may break with an Antigravity update. Two limits are
  built in and said out loud: it only works while Antigravity is open with
  that folder, and it cannot be held read-only, so only Accept edits is
  offered. Stopping a mission stops the watch; Antigravity's agent may keep
  going inside Antigravity.

## 0.12.0 — 2026-09-03

- **Two new routes.** OpenCode, which ships free models that need no sign-in
  at all (Muse Spark, Nemotron, and more), so a fresh install can run a
  mission for nothing the minute it opens; and GitHub Copilot CLI, for anyone
  on a Copilot plan that includes the CLI, offered as Auto so Copilot picks
  the model your plan allows. Both keep the same guarantees as the other
  routes: read-only really is read-only (OpenCode is held by its own
  permission config, Copilot by denying its write and shell tools), a reply
  resumes the same session, and every run is recorded like any other.
- **Meetings.** Write to two or more teammates at once and each gets their
  own run, but your teammate's next turn waits until all of them have
  answered or finished, then starts once with every reply quoted. The thread
  says who it is waiting on and who left without a word.
- Each teammate row now says which runtime and model they are. The roster
  draws the same faces as the sidebar. A banner above the composer says when
  a new version is downloaded and offers the restart.
- The route search folds hyphens and dots, so "muse spark" finds
  `muse-spark-1.3` and "gpt 5" finds `gpt-5.6`.

## 0.11.0 — 2026-09-03

- **Faces that say what a teammate is doing.** Eight states, each with its own
  motion: up thinks, down works, forward talks to you, sideways listens.
  Thinking tilts and looks up with the dots beside it; working bobs with
  weight and looks down; replying looks at you and talks; waiting on you holds
  a stare inside a slow amber ring; a message arriving earns a glance; a
  finished mission earns one hop. Idle and blocked are the only still faces,
  and blocked shuts its eyes behind a red border so it never reads as idle.
- **One teammate, one state, everywhere.** The sidebar row, the workroom
  header and the working line in the thread now resolve the same teammate to
  the same state at the same moment, and the word beside the face is that
  state's word: "thinking", "working", "replying", "waiting on you".
- Reduced motion turns every animation off; state still reads from the text
  and the presence dot.

## 0.10.2 — 2026-09-03

- **Each teammate stays the model you made them.** The route you last started
  a teammate on (runtime, model, mode) is now theirs. When they reply to
  another teammate on their own, they reply on that route, never on whoever
  wrote to them, so Grok argues as Grok and Claude answers as Claude. Picking
  a teammate in the sidebar now also sets the composer to their route.
- A teammate who has never run borrows the sender's route once, and the thread
  says so, with the fix: message them once on the route they should keep.
- Seen live: asked by a Cursor teammate to parrot a passphrase, a Claude
  teammate declined and said why, because teammate messages are delivered as
  claims, not orders. That is the point, and it survives across models.

## 0.10.1 — 2026-09-03

- **Teammates reply to each other by default**, and for as long as the work
  needs. Colin's call, and the right one: talking to each other is the point
  of having more than one teammate. An exchange now ends when a reply has
  nothing more to say, rather than after a fixed two hops; six automatic runs
  is the backstop. Each hop continues that teammate's own conversation, so
  both sides read as one thread. Settings → Teammates now only turns it off.

## 0.10.0 — 2026-09-03

- **Teammates can reply to each other.** Turn it on under Settings → Teammates.
  When one teammate writes to another, Locust starts a run for the recipient
  with the message as its brief, and their answer starts the sender's next
  turn, so it lands in the thread that asked. If that thread is on screen, the
  view follows it to the new turn. Two hops, then it stops and waits for you.
- Each hop is a real run on a real account, so it is capped at two. Each
  hop runs on the sender's runtime, model and mode, is owned by the teammate
  who replied, and is recorded like any other mission. When a reply could not
  start, the thread that shared says why.
- **Auto-update is live.** Installed copies now find new versions on their
  own and install them when you quit. No more installers by hand.

## 0.9.1 — 2026-09-03

- **One conversation, one sidebar row.** Every reply used to appear in the
  sidebar as its own entry, even though the thread showed the exchange as one.
  The sidebar now lists a conversation once, named for what you typed to start
  it, with a small count of how many turns it holds. Clicking it opens the
  newest turn. Nothing changed underneath: each turn is still its own recorded
  run with its own receipt.
- Tool rows in the activity card say what the tool was (`read`, `glob`), so a
  file the teammate read and then edited no longer appears as the same path
  twice with nothing to tell the rows apart.

## 0.9.0 — 2026-09-02

- **See the actual diff.** The activity card used to say a teammate edited a
  file and stop there. Open it now and every changed file is listed with its
  status and `+N −M`; click one and the change itself unfolds inline, line
  numbers, hunk headers, and the exact span that changed on each line.
  Unchanged runs collapse behind a button that names how many lines it hides,
  and an open file always ends by saying whether you have seen all of it.
- Every number on that card is counted from the lines shown, so the total,
  each file, and each hunk header agree. If a change was too large to record
  in full, the footer says so and gives the runtime's own total rather than
  quietly showing less.
- Commands sit in the same list as files, with their exit code, so what a
  teammate did reads top to bottom in the order it happened.
- **Colour means one thing again.** Lime is now only "happening right now".
  Done plan steps, standing permissions, and reachable-but-idle routes moved
  to green, so a glance at the window tells you what is live.
- Cards that need a decision are raised toward you; cards about something
  that already happened sit recessed. Screen titles and the two empty states
  carry real display size.

## 0.8.0 — 2026-09-02

- **Right-click a mission** in the sidebar for Open, Copy mission id, and
  Delete. Delete still asks before it acts, and refuses while the mission is
  running.
- **Search actually searches.** The sidebar's search field had never been
  wired to anything. It now matches a mission's title or its id, so an id
  copied from a receipt finds its mission.
- Deleting a mission from anywhere now updates the Settings storage line
  instead of leaving the number it read at launch.

## 0.7.2 — 2026-09-02

- **Fixed: replying to a teammate started a new mission instead of
  continuing.** On Windows, Cursor cannot be held read-only — its sandbox
  needs macOS or Linux — so every Cursor mission in "Ask" was refused before
  it ran, and a run that never started leaves nothing to continue. Cursor on
  Windows now offers "Accept edits" only, and the menu says why.

## 0.7.0 — 2026-09-02

- **The model list is readable.** Cursor lists every effort of every model
  separately, 217 entries on a real account. They are now one row per model,
  with the efforts in the effort control where they belong.
- **Better order.** Models you have actually run come first, then a shortlist
  of flagship families, then the rest. Nothing is hidden.
- **"Approve each action" is no longer offered where it cannot run.** It works
  on Codex CLI only; picking it with another route used to refuse every
  message you sent.
- The Missions screen's "Running" filter now matches running missions, and its
  rows are titled with what you typed rather than a machine-written briefing.

## 0.6.0 — 2026-09-02

- **Updates.** Locust checks for a new version shortly after launch and
  downloads it quietly. It never installs on its own, and it will not install
  while a mission is running. Settings has a Check now button.

## 0.5.0 — 2026-09-02

- Cursor missions can no longer claim to be read-only when nothing enforces
  it, and the ledger no longer records a failed command as a successful one.
- Discovery no longer re-runs a full probe sweep on every mission start.
- A mission whose options cannot be turned into a command records nothing at
  all, instead of leaving a permanent file for a run that never happened.

## 0.4.1 — 2026-09-02

- **Fixed: replies on Codex never worked.** `codex exec resume` rejects two
  of the arguments Locust was passing, so every Codex follow-up failed. Both
  turns now share one session, verified on a real run.
- A run that failed before it started no longer says "Starting…" forever, and
  the next thing you type starts a fresh mission rather than an error.

## 0.4.0 — 2026-09-02

- **Retention.** Settings shows what your history costs and offers to delete
  finished missions older than 30 days, 90 days or a year. Nothing is ever
  deleted on a timer; the first press only previews, and a mission an ongoing
  conversation continues from is kept even when it is old.
- The route picker no longer lets one runtime's long model list bury the
  others.

## 0.3.0 — 2026-09-02

- **Cursor Agent is a third runtime**, with its own models read from its own
  CLI. Missions run, stream, and resume on it.
- Gemini CLI is found but cannot run: Google stopped serving it to consumer
  accounts in June 2026. Gemini models are reachable through Cursor.

## 0.2.0 — 2026-09-01

- **Delete a mission** for good from the workroom header.
- Settings shows which build you are on.

## 0.1.0 — 2026-09-01

First installable build: Codex CLI and Claude Code as selectable runtimes,
teammates, missions recorded to a durable local ledger, handoffs between
runtimes, and a workroom where teammates pass findings to each other.
