import {test} from 'node:test';
import assert from 'node:assert/strict';
import {validateOutputInput} from '../src/output.ts';
test('PDF protection requires a matching nonempty password',()=>{
  const settings={mode:'target',target_mb:'10',protect:true};
  assert.throws(()=>validateOutputInput(settings,'',''));
  assert.throws(()=>validateOutputInput(settings,'secret','different'));
  assert.doesNotThrow(()=>validateOutputInput(settings,'日本語-test','日本語-test'));
});
test('target MB uses finite positive decimal precision',()=>{
  for(const value of ['0','-1','NaN','1e2','0.0000001','']) {
    assert.throws(()=>validateOutputInput({mode:'target',target_mb:value,protect:false},'',''));
  }
  assert.doesNotThrow(()=>validateOutputInput({mode:'target',target_mb:'0.000001',protect:false},'',''));
});
