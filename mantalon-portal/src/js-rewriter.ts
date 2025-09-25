// @ts-ignore: TS2580
const recast = require("recast");
import { namedTypes as n, builders as b } from "ast-types";

function wrap(node: any) {
    const wrapped = b.callExpression(b.identifier("mantalon"), [node]);
    return wrapped;
}

function rewriteCalls(node: any, parent?: any): any {
    if (!node || typeof node !== "object") return node;

    switch (node.type) {
        case "CallExpression":
            if (node.callee.type === "MemberExpression") {
                // Transform obj.method(args) → obj.method.call(obj, ...args)
                const objectTransformed = rewriteCalls(node.callee.object, node);
                node.arguments.map((child: any) => rewriteCalls(child, node));
                node.arguments.unshift(objectTransformed);
                node.callee = b.memberExpression(
                    rewriteCalls(node.callee, node),
                    b.identifier("call"),
                    false
                );
                return node;
            } else {
                node.callee = rewriteCalls(node.callee, node);
                node.arguments = node.arguments.map((arg: any) => rewriteCalls(arg, node));
                return node;
            }

        case "MemberExpression":
            node.object = rewriteCalls(node.object, node);
            node.property = rewriteCalls(node.property, node);
            return node;

        default:
            // recurse through all child nodes
            for (const key of Object.keys(node)) {
                if (Array.isArray(node[key])) {
                    node[key] = node[key].map((child: any) => rewriteCalls(child, node));
                } else if (node[key] && typeof node[key] === "object" && node[key].type) {
                    node[key] = rewriteCalls(node[key], node);
                }
            }
            return node;
    }
}

function wrapExpressions(node: any, parent?: any): any {
    if (!node || typeof node !== "object") return node;

    switch (node.type) {
        case "Identifier":
            return wrap(node);
        
        case "Property":
        case "Keyword":
        case "Punctuator":
            return node;

        case "CallExpression":
            node.callee = wrapExpressions(node.callee, node);
            node.object = wrapExpressions(node.object, node);
            node.arguments = node.arguments.map((child: any) => wrapExpressions(child, node));
            return node;

        case "VariableDeclaration":
            node.declarations = node.declarations.map((child: any) => wrapExpressions(child, node));
            return node;

        case "VariableDeclarator":
            node.init = wrapExpressions(node.init, node);
            return node;

        case "AssignmentExpression":
            // TODO: Maybe this is leaking the top-level data
            if (node.left.type == "MemberExpression") {
                node.left.object = wrapExpressions(node.left.object, node.left);
            }
            node.right = wrapExpressions(node.right, node);
            return node;

        case "ArrowFunctionExpression":
        case "FunctionDeclaration":
        case "FunctionExpression":
        case "CatchClause":
            node.body = wrapExpressions(node.body, node);
            return node;
        
        case "Module":
        case "Program":
        case "BlockStatement":
            node.body = node.body.map((child: any) => wrapExpressions(child, node));
            return node;
        
        case "ExpressionStatement":
            node.expression = wrapExpressions(node.expression, node);
            return node;

        case "MemberExpression":
            node.object = wrapExpressions(node.object, node);
            node.property = node.computed ? wrapExpressions(node.property, node) : node.property;

            // Prevent mantalon(<ident>.call)(<ident>)
            if (parent && (parent.type === "CallExpression" && parent.callee === node)) {
                return node;
            }

            return wrap(node);

        default:
            // console.log("unsupported", node);

            for (const key of Object.keys(node)) {
                if (Array.isArray(node[key])) {
                    node[key] = node[key].map((child: any) => wrapExpressions(child, node));
                } else if (node[key] && typeof node[key] === "object" && node[key].type) {
                    node[key] = wrapExpressions(node[key], node);
                }
            }
            return node;
    }
}


export function applyJsProxyOnJs(input: string): string {
    let ast = recast.parse(input);
    rewriteCalls(ast);
    let restringified = recast.print(ast).code;
    
    ast = recast.parse(restringified);
    wrapExpressions(ast);


    return recast.print(ast).code;
}
