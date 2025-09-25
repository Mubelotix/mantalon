// @ts-ignore: TS2580
const recast = require("recast");
import * as types from "ast-types";
import { NodePath } from "ast-types/lib/node-path";

function isBindingIdentifier(path: NodePath<types.namedTypes.Identifier>) {
    let node = path.node;
    let parentPath = path.parentPath;

    while (parentPath) {
        const parent = parentPath.node;

        if (!parent) break;

        if (
            // variable declaration
            (parent.type === "VariableDeclarator" && parent.id === node) ||
            // function name
            (parent.type === "FunctionDeclaration" && parent.id === node) ||
            // function parameters
            ((parent.type === "FunctionDeclaration" || parent.type === "FunctionExpression" || parent.type === "ArrowFunctionExpression") && parent.params.includes(node)) ||
            // object destructuring property key or value
            (parent.type === "Property" &&
             ((parent.key === node && !parent.computed) ||
              (parent.value === node && parentPath.parent?.node?.type === "ObjectPattern"))) ||
            // array pattern
            parent.type === "ArrayPattern"
        ) {
            return true;
        }

        node = parent;
        parentPath = parentPath.parentPath;
    }

    return false;
}

export function applyJsProxyOnJs(input: string): string {
    const b = recast.types.builders;
    const ast = recast.parse(input);

    function wrap(node: any) {
        return b.callExpression(b.identifier("mantalon"), [node]);
    }

    recast.types.visit(ast, {
        visitMemberExpression(path: NodePath<types.namedTypes.MemberExpression>) {
            this.traverse(path);

            const node = path.node;
            const obj = node.object.type === "CallExpression"
                && node.object.callee.type === "Identifier"
                && node.object.callee.name === "mantalon"
                ? node.object
                : wrap(node.object);

            path.replace(wrap(b.memberExpression(obj, node.property, node.computed)));
            return false;
        },


        visitCallExpression(path: NodePath<types.namedTypes.CallExpression>) {
            this.traverse(path);

            const node = path.node;

            // wrap callee if not already wrapped
            if (
                node.callee.type !== "CallExpression" ||
                !(
                    node.callee.callee.type === "Identifier" &&
                    node.callee.callee.name === "mantalon"
                )
            ) {
                node.callee = wrap(node.callee);
            }

            return false;
        },

        visitIdentifier(path: NodePath<types.namedTypes.Identifier>) {
            const parent = path.parent.node;

            // Skip identifiers that are keys in member expressions (non-computed)
            if (parent.type === "MemberExpression" && parent.property === path.node && !parent.computed) {
                return false;
            }

            if (isBindingIdentifier(path)) return false;

            path.replace(wrap(path.node));
            return false;
        }
    });

    return recast.print(ast).code;
}
