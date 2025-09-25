const recast = require("recast");

function isBindingIdentifier(path) {
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
        visitMemberExpression(path) {
            // first, traverse children
            this.traverse(path);

            // rebuild the full member expression with wrapping
            const node = path.node;

            // wrap the object if not already wrapped
            const obj = node.object.type === "CallExpression" && node.object.callee.name === "mantalon"
                ? node.object
                : wrap(node.object);

            // wrap the full member expression (object.property)
            path.replace(wrap(b.memberExpression(obj, node.property, node.computed)));

            return false;
        },

        visitCallExpression(path) {
            this.traverse(path);

            const node = path.node;

            // wrap callee if not already wrapped
            if (node.callee.type !== "CallExpression" || node.callee.callee.name !== "mantalon") {
                node.callee = wrap(node.callee);
            }

            return false;
        },

        visitIdentifier(path) {
            if (isBindingIdentifier(path)) return false;

            path.replace(wrap(path.node));
            return false;
        }
    });

    return recast.print(ast).code;
}
