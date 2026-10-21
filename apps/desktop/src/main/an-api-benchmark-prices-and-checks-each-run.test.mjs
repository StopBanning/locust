import { describe, expect, it } from 'vitest'
import { writeFile, rm, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { PRICES, estimateCost, spendStop, receipt, resultRow, csv, summary, options, ownModelRoute, answerOf } from '../../../../_tools/api-benchmark.mjs'
import { TASKS, fixture, objective } from '../../../../_tools/api-benchmark-tasks.mjs'

describe('an API benchmark prices and checks each run', () => {
  it('uses explicit standard API rates, the conservative Haiku tier and separately priced caches', () => {
    expect(PRICES['claude-opus-5-5']).toEqual({ input:4, output:20, read:0.2, write:8 })
    expect(PRICES['claude-sonnet-5-5'].input).toBe(2)
    expect(PRICES['claude-haiku-5-5']).toMatchObject({ input:0.5, output:2.5, under100k:{input:0.1, output:0.5} })
    expect(estimateCost('own-123/claude-opus-5-5',{inputTokens:1e6,outputTokens:1e6,cacheReadTokens:1e6,cacheWriteTokens:1e6})).toBe(32.2)
    expect(estimateCost('claude-haiku-5-5',{inputTokens:1e6,outputTokens:1e6})).toBe(3)
    expect(estimateCost('opencode/free-free',undefined)).toBe(0)
    expect(estimateCost('unknown',{inputTokens:1,outputTokens:1})).toBeNull()
    expect(estimateCost('claude-opus-5-5',{inputTokens:1})).toBeNull()
    expect(estimateCost('claude-opus-5-5',{inputTokens:-1,outputTokens:1})).toBeNull()
  })
  it('stops before another group at the spend boundary and fails closed on unknown paid usage', () => {
    expect(spendStop([1,2],4)).toBeUndefined()
    expect(spendStop([1,2],3)).toContain('No new turns')
    expect(spendStop([4],3)).toContain('No new turns')
    expect(spendStop([null],20)).toContain('unknown')
    expect(spendStop([NaN],20)).toContain('unknown')
    expect(spendStop([],0)).toContain('positive')
  })
  it('keeps objective checks before scores and failed or unreported measurements null, not zero', () => {
    const mission={missionId:'m1',phase:'completed',events:[{type:'run.started',occurredAt:'2026-01-01T00:00:00Z'},{type:'run.completed',occurredAt:'2026-01-01T00:00:02Z',payload:{usage:{inputTokens:100,outputTokens:20}}}]}
    const row=resultRow({task:'bug',model:'claude-opus-5-5',actualModel:'own-123/claude-opus-5-5',run:2,compareId:'cmp_1',mission,check:{passed:false,exitCode:1,said:'failed'},points:2,pick:true,standIn:false})
    expect(row).toMatchObject({task:'bug',run:2,checkPassed:false,checkExitCode:1,pointsMet:2,panelPick:true,inputTokens:100,outputTokens:20,wallMs:2000,estimatedDollars:0.0008,missionId:'m1'})
    expect(row.estimateLabel).toContain('Estimate')
    expect(csv([row]).indexOf('checkPassed')).toBeLessThan(csv([row]).indexOf('pointsMet'))
    expect(resultRow({task:'bug',model:'claude-opus-5-5',actualModel:'claude-opus-5-5',run:1,mission:{phase:'failed'},points:3,pick:true})).toMatchObject({phase:'failed',checkPassed:null,pointsMet:null,panelPick:null,inputTokens:null,outputTokens:null,wallMs:null,estimatedDollars:null})
    expect(receipt(undefined,'free-free').estimatedDollars).toBe(0)
    expect(summary([row],{charges:[0.1],bases:[],frames:['01.png']})).toContain('Objective checks passed / attempted')
  })
  it('refuses paid judges in stand-in mode and invalid limits before starting anything', () => {
    expect(options(['--stand-in','--tasks','bug,html','--runs','2'])).toMatchObject({standIn:true,tasks:['bug','html'],runs:2,judges:'free',maximum:20})
    expect(()=>options(['--stand-in','--judges','api'])).toThrow()
    expect(()=>options(['--runs','0'])).toThrow()
    expect(()=>options(['--max-dollars','NaN'])).toThrow()
    expect(()=>options(['--surprise'])).toThrow()
    expect(ownModelRoute({ownId:'12345678',model:'claude-opus-5-5',name:'Opus'})).toEqual({runtime:'opencode',model:'own-12345678/claude-opus-5-5',label:'Opus'})
  })
  it('reads normalized replacement text once and measures wall time even when no run-start event was emitted', () => {
    const mission={createdAt:'2026-01-01T00:00:00Z',events:[{type:'message.delta',payload:{itemId:'a',text:'SCORE',operation:'append'}},{type:'message.delta',payload:{itemId:'a',text:'SCORE A 1: met.',operation:'replace'}},{type:'run.completed',occurredAt:'2026-01-01T00:00:03Z',payload:{}}]}
    expect(answerOf(mission)).toBe('SCORE A 1: met.')
    expect(receipt(mission,'free-free').wallMs).toBe(3000)
  })
  it('refuses a check release from another frame or malformed ids before touching the runner', async () => {
    const source=await readFile(new URL('./index.ts',import.meta.url),'utf8')
    const handler=source.match(/ipcMain\.handle\(COMPARE_RERUN_CHECKED_CHANNEL, (async[\s\S]*?\n    \})\)/)?.[1]
    expect(handler).toBeDefined()
    let calls=0
    const fn=new Function('fromOwnWindow','compareReruns','return ('+handler.replaceAll(': unknown','')+')')((event)=>event.own===true,{checked:()=>{calls+=1;return true}})
    expect(await fn({own:false},'origin','run')).toBe(false)
    expect(await fn({own:true},123,'run')).toBe(false)
    expect(await fn({own:true},'origin',null)).toBe(false)
    expect(calls).toBe(0)
    expect(await fn({own:true},'origin','run')).toBe(true)
    expect(calls).toBe(1)
  })
  it('fixes the files and base for every run, and each oracle rejects broken work but accepts a known solution', async () => {
    const solutions={
      bug:{'tags.mjs':"export const normalizeTags = tags => [...new Set(tags.map(tag=>tag.trim().toLowerCase()))]\n"},
      feature:{'paging.mjs':"export function paginate(items,page,size){if(!Number.isInteger(page)||page<1||!Number.isInteger(size)||size<1)throw new RangeError('bounds');return items.slice((page-1)*size,page*size)}\n",'paging.test.mjs':"import {test} from 'node:test';import assert from 'node:assert/strict';import {paginate} from './paging.mjs';test('pages',()=>assert.deepEqual(paginate([1,2,3],2,2),[3]));\n"},
      refactor:{'totals.mjs':"export function parseRow(row){const p=row.split(':');if(p.length!==2||!p[0].trim()||!p[1].trim()||!Number.isFinite(Number(p[1])))return null;return {name:p[0].trim(),amount:Number(p[1])}}\nexport function summarize(rows){const r={};for(const {name,amount} of rows)r[name]=(r[name]??0)+amount;return r}\nexport const total=text=>summarize(text.split('\\n').map(parseRow).filter(Boolean));\n"},
      tests:{'clamp.test.mjs':"import {test} from 'node:test';import assert from 'node:assert/strict';import {clamp} from './clamp.mjs';test('bounds',()=>{assert.equal(clamp(20,0,10),10);assert.equal(clamp(-1,0,10),0)});\n"},
      html:{'index.html':'<!doctype html><html lang="en"><head><title>Notes</title><link rel="stylesheet" href="style.css"></head><body><main><h1>Project notes</h1><button type="button" onclick="const d=document.getElementById(\'details\');d.hidden=!d.hidden">Toggle details</button><p id="details" hidden>Small steps, real checks.</p></main></body></html>\n'}
    }
    const roots=[]
    try {
      const first=await fixture('bug'),same=await fixture('bug');roots.push(first.root,same.root)
      expect(first.commit).toBe(same.commit)
      expect(first.hash).toBe(same.hash)
      for(const id of Object.keys(TASKS)) {
        const fixed=await fixture(id);roots.push(fixed.root)
        expect((await objective(id,fixed.root)).passed,id+' broken').toBe(false)
        for(const [name,text] of Object.entries(solutions[id]))await writeFile(join(fixed.root,name),text)
        const result=await objective(id,fixed.root)
        expect(result.passed,id+': '+result.said).toBe(true)
      }
    } finally {for(const root of roots)await rm(root,{recursive:true,force:true})}
  },30_000)
})
