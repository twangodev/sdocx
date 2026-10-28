"""AST audit for V2 runpy, CanvasProbe, and extracted Renderer adapter interfaces."""
import ast
from pathlib import Path


def _targets(node):
    if isinstance(node,ast.Name): return {node.id}
    if isinstance(node,(ast.Tuple,ast.List)):
        out=set()
        for item in node.elts: out|=_targets(item)
        return out
    return set()


def _top_level_names(tree):
    names=set()
    for n in tree.body:
        if isinstance(n,(ast.FunctionDef,ast.AsyncFunctionDef,ast.ClassDef)): names.add(n.name)
        elif isinstance(n,(ast.Import,ast.ImportFrom)):
            for item in n.names: names.add(item.asname or item.name.split('.')[0])
        elif isinstance(n,ast.Assign):
            for t in n.targets:names|=_targets(t)
        elif isinstance(n,ast.AnnAssign): names|=_targets(n.target)
    return names


def _instance_members(class_node):
    result={n.name for n in class_node.body if isinstance(n,(ast.FunctionDef,ast.AsyncFunctionDef))}
    for n in ast.walk(class_node):
        if isinstance(n,(ast.Assign,ast.AnnAssign)):
            targets=n.targets if isinstance(n,ast.Assign) else [n.target]
            for target in targets:
                if isinstance(target,ast.Attribute) and isinstance(target.value,ast.Name) and target.value.id=='self':
                    result.add(target.attr)
    return result


def _root_attributes(tree, root):
    out=set()
    for n in ast.walk(tree):
        if isinstance(n,ast.Attribute):
            x=n
            while isinstance(x,ast.Attribute): x=x.value
            if isinstance(x,ast.Name) and x.id==root:
                first=n
                while isinstance(first.value,ast.Attribute): first=first.value
                if isinstance(first.value,ast.Name) and first.value.id==root: out.add(first.attr)
    return out


def _subscript_keys(tree, root):
    return {n.slice.value for n in ast.walk(tree)
            if isinstance(n,ast.Subscript) and isinstance(n.value,ast.Name) and n.value.id==root
            and isinstance(n.slice,ast.Constant) and isinstance(n.slice.value,str)}


def audit_runtime_interface(candidate_path, runner_path, fountain_path, canvas_path, marker_path, texture_path, saved_path):
    candidate=ast.parse(Path(candidate_path).read_text(encoding='utf-8'))
    runner=ast.parse(Path(runner_path).read_text(encoding='utf-8'))
    fountain=ast.parse(Path(fountain_path).read_text(encoding='utf-8'))
    canvas=ast.parse(Path(canvas_path).read_text(encoding='utf-8'))
    marker=ast.parse(Path(marker_path).read_text(encoding='utf-8'))
    texture=ast.parse(Path(texture_path).read_text(encoding='utf-8'))
    saved=ast.parse(Path(saved_path).read_text(encoding='utf-8'))
    runner_names=_top_level_names(runner)
    ns_keys=_subscript_keys(candidate,'ns')
    missing_ns=ns_keys-runner_names
    if missing_ns: raise ValueError(f'runpy keys are not defined by V2 runner: {sorted(missing_ns)}')
    base=next(n for n in fountain.body if isinstance(n,ast.ClassDef) and n.name=='NativeFountain')
    child=next(n for n in canvas.body if isinstance(n,ast.ClassDef) and n.name=='CanvasProbe')
    inherited=_instance_members(base)|_instance_members(child)
    runner_p_attrs=set()
    for n in ast.walk(runner):
        if isinstance(n,(ast.Assign,ast.AnnAssign)):
            targets=n.targets if isinstance(n,ast.Assign) else [n.target]
            for t in targets:
                if isinstance(t,ast.Attribute) and isinstance(t.value,ast.Name) and t.value.id=='p': runner_p_attrs.add(t.attr)
    p_used=_root_attributes(candidate,'p')
    missing_p=p_used-(inherited|runner_p_attrs)
    if missing_p: raise ValueError(f'candidate p interface is not defined by runtime/classes/runner: {sorted(missing_p)}')
    adapter_class=next(n for n in marker.body if isinstance(n,ast.ClassDef) and n.name=='P')
    adapter_members=_instance_members(adapter_class)
    texture_class=next(n for n in texture.body if isinstance(n,ast.ClassDef) and n.name=='TextureOracle')
    saved_class=next(n for n in saved.body if isinstance(n,ast.ClassDef) and n.name=='Oracle')
    adapter_members|=_instance_members(texture_class)|_instance_members(saved_class)|_instance_members(base)
    runner_adapter_attrs=set()
    for n in ast.walk(runner):
        if isinstance(n,(ast.Assign,ast.AnnAssign)):
            targets=n.targets if isinstance(n,ast.Assign) else [n.target]
            for t in targets:
                if isinstance(t,ast.Attribute) and isinstance(t.value,ast.Name) and t.value.id=='_adapter': runner_adapter_attrs.add(t.attr)
    adapter_used=_root_attributes(candidate,'adapter')
    missing_adapter=adapter_used-(adapter_members|runner_adapter_attrs)
    if missing_adapter: raise ValueError(f'candidate adapter interface is not defined by extracted P/runner: {sorted(missing_adapter)}')
    return {'runpy_keys':sorted(ns_keys),'p_attributes':sorted(p_used),'adapter_attributes':sorted(adapter_used)}
